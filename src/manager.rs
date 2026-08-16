//! High-level `Manager` facade: one-stop add/list/remove/update over an injectable [`Env`].
//!
//! The manager is pure data: it returns structured outcomes and never prints or exits;
//! the CLI layer (src/commands) is responsible for rendering.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use serde::Serialize;

use crate::core::agents::{
    AGENTS, Agent, Env, agent_display, config_home, detect_installed_agents,
    ensure_universal_agents, get_agent, global_skills_dir, home,
};
use crate::core::discover::{Skill, discover_skills, filter_skills};
use crate::core::fetch::{clone_repo, download_and_extract};
use crate::core::install::{
    InstallMode, find_skill, get_canonical_path, get_install_path, install_skill_for_agent,
    list_installed_skills, matches_skill, resolve_to_remove, sanitize_name, scan_installed,
};
use crate::core::lock::{
    LockEntry, compute_folder_hash, find_lock_entry, global_lock_path, local_lock_path,
    lock_fields, read_local_lock, write_local_lock,
};
use crate::core::source::{Source, SourceType, parse_source};
use crate::error::{Result, SkillsError};

/// Skill manager: carries injectable context and runs add/list/remove/update.
pub struct Manager {
    env: Env,
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager {
    /// Build a manager from the real environment (home / config / cwd).
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Start customizing a manager (inject home/config/cwd/env vars).
    pub fn builder() -> ManagerBuilder {
        ManagerBuilder::default()
    }

    /// Access the resolved environment context.
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Add (install) skills from a source. Returns discovered + installed + failed.
    pub fn add(&self, req: &AddRequest) -> Result<AddOutcome> {
        let parsed = parse_source(&req.source)?;
        let include_internal = !req.skills.is_empty();

        // Fetch skills (the temp dir is held until install finishes).
        let skills: Vec<Skill>;
        let _temp: Option<tempfile::TempDir>;
        match parsed.ty {
            SourceType::Local => {
                let path = parsed
                    .local_path
                    .as_ref()
                    .ok_or_else(|| SkillsError::msg("local source missing path"))?;
                if !path.exists() {
                    return Err(SkillsError::msg(format!(
                        "Local path does not exist: {}",
                        path.display()
                    )));
                }
                skills = discover_skills(
                    path,
                    parsed.subpath.as_deref(),
                    req.full_depth,
                    include_internal,
                )?;
                _temp = None;
            }
            SourceType::Download | SourceType::WellKnown => {
                let (t, root) = download_and_extract(&parsed.url)?;
                skills = discover_skills(
                    &root,
                    parsed.subpath.as_deref(),
                    req.full_depth,
                    include_internal,
                )?;
                _temp = Some(t);
            }
            _ => {
                let tmp = clone_repo(&parsed.url, parsed.r#ref.as_deref())?;
                skills = discover_skills(
                    tmp.path(),
                    parsed.subpath.as_deref(),
                    req.full_depth,
                    include_internal,
                )?;
                _temp = Some(tmp);
            }
        }

        if skills.is_empty() {
            return Err(SkillsError::msg(
                "No valid skills found. Skills require a SKILL.md with name and description.",
            ));
        }

        // --list: report discovered skills without installing.
        if req.list_only {
            return Ok(AddOutcome {
                source: parsed,
                skills,
                selected: Vec::new(),
                target_agents: Vec::new(),
                installed: Vec::new(),
                failed: Vec::new(),
                list_only: true,
            });
        }

        // Select skills.
        let selected: Vec<Skill> = if req.skills.iter().any(|s| s == "*") {
            skills.clone()
        } else if !req.skills.is_empty() {
            filter_skills(&skills, &req.skills)
        } else {
            skills.clone()
        };

        // Determine target agents.
        let target_agents: Vec<&'static Agent> = if req.agents.iter().any(|a| a == "*") {
            AGENTS.iter().collect()
        } else if !req.agents.is_empty() {
            let mut agents = Vec::new();
            let mut invalid = Vec::new();
            for name in &req.agents {
                match get_agent(name) {
                    Some(a) => agents.push(a),
                    None => invalid.push(name.clone()),
                }
            }
            if !invalid.is_empty() {
                return Err(SkillsError::InvalidAgents(invalid.join(", ")));
            }
            agents
        } else {
            let installed = detect_installed_agents(&self.env);
            ensure_universal_agents(installed)
        };

        let mode = if req.copy {
            InstallMode::Copy
        } else {
            InstallMode::Symlink
        };

        // Install.
        let mut installed: Vec<InstallSuccess> = Vec::new();
        let mut failed: Vec<InstallFailure> = Vec::new();
        for skill in &selected {
            for agent in &target_agents {
                let r = install_skill_for_agent(skill, agent, &self.env, req.global, mode);
                if r.success {
                    installed.push(InstallSuccess {
                        name: skill.name.clone(),
                        agent: agent.display.to_string(),
                        canonical_path: r.canonical_path.clone(),
                    });
                } else {
                    failed.push(InstallFailure {
                        skill: skill.name.clone(),
                        agent: agent.display.to_string(),
                        error: r.error.clone().unwrap_or_default(),
                    });
                }
            }
        }

        // Write the lock (only for successfully installed skills).
        if !installed.is_empty() {
            write_lock(&parsed, &selected, &installed, req.global, &self.env)?;
        }

        Ok(AddOutcome {
            source: parsed,
            skills,
            selected,
            target_agents: target_agents
                .iter()
                .map(|a| a.display.to_string())
                .collect(),
            installed,
            failed,
            list_only: false,
        })
    }

    /// List installed skills (project/global), enriched with lock metadata.
    pub fn list(&self, req: &ListRequest) -> Result<Vec<ListedSkill>> {
        let invalid: Vec<String> = req
            .agents
            .iter()
            .filter(|a| get_agent(a).is_none())
            .cloned()
            .collect();
        if !invalid.is_empty() {
            return Err(SkillsError::InvalidAgents(invalid.join(", ")));
        }

        let installed = list_installed_skills(&self.env, req.global, &req.agents);
        let lock = read_local_lock(&lock_path(&self.env, req.global));
        let mut out = Vec::new();
        for s in &installed {
            let entry = find_lock_entry(&lock, &s.name);
            out.push(ListedSkill {
                name: s.name.clone(),
                path: s.canonical_path.clone(),
                scope: s.scope.clone(),
                agents: s.agents.iter().map(|a| agent_display(a)).collect(),
                source: entry.map(|e| e.source.clone()),
                source_url: entry.and_then(|e| e.source_url.clone()),
                source_type: entry.map(|e| e.source_type.clone()),
            });
        }
        Ok(out)
    }

    /// Remove installed skills.
    pub fn remove(&self, req: &RemoveRequest) -> Result<RemoveOutcome> {
        let global = req.global;

        // Validate agents.
        let invalid: Vec<String> = req
            .agents
            .iter()
            .filter(|a| get_agent(a).is_none())
            .cloned()
            .collect();
        if !invalid.is_empty() {
            return Err(SkillsError::InvalidAgents(invalid.join(", ")));
        }

        let installed = scan_installed(&self.env, global);

        // List-only mode (no skills and not --all).
        if req.skills.is_empty() && !req.all {
            return Ok(RemoveOutcome {
                installed,
                requested: Vec::new(),
                removed: Vec::new(),
            });
        }

        // Resolve the skill names to remove (lock keys take priority, then on-disk dir names).
        let lock = read_local_lock(&lock_path(&self.env, global));
        let lock_keys: Vec<String> = lock.skills.keys().cloned().collect();
        let mut requested: Vec<String> = if req.all {
            installed.iter().chain(lock_keys.iter()).cloned().collect()
        } else {
            req.skills.clone()
        };
        if !req.skill.is_empty() {
            requested.extend(req.skill.iter().cloned());
        }
        if requested.is_empty() {
            return Ok(RemoveOutcome {
                installed,
                requested: Vec::new(),
                removed: Vec::new(),
            });
        }

        let selected = resolve_to_remove(&requested, &installed, &lock_keys);
        if selected.is_empty() {
            return Ok(RemoveOutcome {
                installed,
                requested,
                removed: Vec::new(),
            });
        }

        // Target agents (default all, ensuring ghost symlinks are cleaned).
        let target_agents: Vec<&'static Agent> = if req.agents.is_empty() {
            AGENTS.iter().collect()
        } else {
            req.agents.iter().filter_map(|a| get_agent(a)).collect()
        };

        let mut removed: Vec<String> = Vec::new();
        for name in &selected {
            let canonical = get_canonical_path(name, global, &self.env);
            let sanitized = sanitize_name(name);

            for agent in &target_agents {
                let skill_path = get_install_path(name, agent, global, &self.env);
                // Skip canonical (handled at the end).
                if skill_path == canonical {
                    continue;
                }
                let _ = std::fs::remove_dir_all(&skill_path);
                let _ = std::fs::remove_file(&skill_path);
                // Clean up the legacy location.
                if global {
                    if let Some(base) = global_skills_dir(agent, &self.env) {
                        let _ = std::fs::remove_dir_all(base.join(&sanitized));
                    }
                } else {
                    let _ = std::fs::remove_dir_all(
                        self.env.cwd.join(agent.skills_dir).join(&sanitized),
                    );
                }
            }

            // Delete canonical only when no other agent still uses it.
            let still_used = AGENTS.iter().any(|a| {
                let p = get_install_path(name, a, global, &self.env);
                p != canonical && p.exists()
            });
            if !still_used {
                let _ = std::fs::remove_dir_all(&canonical);
            }

            // Clean the lock.
            let mut lock = read_local_lock(&lock_path(&self.env, global));
            lock.version = 1;
            lock.skills.remove(name);
            lock.skills.remove(&sanitized);
            if let Some(parent) = lock_path(&self.env, global).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = write_local_lock(&lock, &lock_path(&self.env, global));

            removed.push(name.clone());
        }

        Ok(RemoveOutcome {
            installed,
            requested,
            removed,
        })
    }

    /// Update installed skills from their recorded (non-local) sources.
    pub fn update(&self, req: &UpdateRequest) -> Result<UpdateOutcome> {
        let global = resolve_scope(req, &self.env);
        let lock_path = lock_path(&self.env, global);
        let lock = read_local_lock(&lock_path);
        let skills: Vec<(String, LockEntry)> = lock
            .skills
            .iter()
            .filter(|(name, entry)| {
                matches_skill(name, &req.skills) && entry.source_type != "local"
            })
            .map(|(n, e)| (n.clone(), e.clone()))
            .collect();

        if skills.is_empty() {
            return Ok(UpdateOutcome {
                global,
                ..Default::default()
            });
        }

        // Group by source (same source is cloned only once).
        let mut by_source: BTreeMap<String, Vec<(String, LockEntry)>> = BTreeMap::new();
        for (name, entry) in skills {
            by_source
                .entry(entry.source.clone())
                .or_default()
                .push((name, entry));
        }

        let mut outcome = UpdateOutcome {
            global,
            ..Default::default()
        };
        for (source, items) in &by_source {
            let first = &items[0].1;
            let clone_url = first.source_url.clone().unwrap_or_else(|| source.clone());
            let r#ref = first.r#ref.clone();
            let parsed = parse_source(&clone_url)?;

            let tmp = match clone_repo(&parsed.url, r#ref.as_deref()) {
                Ok(t) => t,
                Err(e) => {
                    for (name, _) in items {
                        outcome.failures.push(format!("{name}: {e}"));
                        outcome.failed += 1;
                    }
                    continue;
                }
            };
            let discovered = discover_skills(tmp.path(), parsed.subpath.as_deref(), true, true)
                .unwrap_or_default();

            for (name, entry) in items {
                let target = find_skill(&discovered, name, entry.skill_path.as_deref());
                let Some(skill) = target else {
                    outcome
                        .failures
                        .push(format!("Skill '{name}' not found in {source}"));
                    outcome.failed += 1;
                    continue;
                };
                let detected = detect_installed_agents(&self.env);
                let agents = ensure_universal_agents(detected);
                for agent in &agents {
                    let r = install_skill_for_agent(
                        skill,
                        agent,
                        &self.env,
                        global,
                        InstallMode::Symlink,
                    );
                    if r.success {
                        outcome.updated += 1;
                    } else {
                        outcome.failed += 1;
                    }
                }
                outcome.updated_names.push(name.clone());
            }
        }

        Ok(outcome)
    }
}

/// Chained builder for [`Manager`], injecting home/config/cwd/env vars.
#[derive(Default)]
pub struct ManagerBuilder {
    home: Option<PathBuf>,
    config: Option<PathBuf>,
    cwd: Option<PathBuf>,
    vars: std::collections::HashMap<String, String>,
}

impl ManagerBuilder {
    pub fn home(mut self, p: impl Into<PathBuf>) -> Self {
        self.home = Some(p.into());
        self
    }

    pub fn config(mut self, p: impl Into<PathBuf>) -> Self {
        self.config = Some(p.into());
        self
    }

    pub fn cwd(mut self, p: impl Into<PathBuf>) -> Self {
        self.cwd = Some(p.into());
        self
    }

    pub fn env_var(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.vars.insert(k.into(), v.into());
        self
    }

    pub fn build(self) -> Manager {
        let cwd = self
            .cwd
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let mut env = Env::new(
            self.home.unwrap_or_else(home),
            self.config.unwrap_or_else(config_home),
            cwd,
        );
        if !self.vars.is_empty() {
            env.set_vars(self.vars);
        }
        Manager { env }
    }
}

// ============================ Request types (clap-free) ============================

#[derive(Debug, Clone, Default)]
pub struct AddRequest {
    pub source: String,
    pub global: bool,
    /// "*" or specific agent names; empty = auto-detect.
    pub agents: Vec<String>,
    /// "*" or specific skill names; empty = all.
    pub skills: Vec<String>,
    pub list_only: bool,
    pub copy: bool,
    pub full_depth: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ListRequest {
    pub global: bool,
    pub agents: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RemoveRequest {
    /// Positional skill names.
    pub skills: Vec<String>,
    /// `--skill` names.
    pub skill: Vec<String>,
    pub global: bool,
    pub agents: Vec<String>,
    pub all: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateRequest {
    pub skills: Vec<String>,
    pub global: bool,
    pub project: bool,
}

// ============================ Outcome types ============================

#[derive(Debug)]
pub struct AddOutcome {
    pub source: Source,
    /// All discovered skills.
    pub skills: Vec<Skill>,
    /// Selected skills (empty when `list_only`).
    pub selected: Vec<Skill>,
    /// Target agent display names.
    pub target_agents: Vec<String>,
    pub installed: Vec<InstallSuccess>,
    pub failed: Vec<InstallFailure>,
    pub list_only: bool,
}

#[derive(Debug)]
pub struct InstallSuccess {
    pub name: String,
    pub agent: String,
    pub canonical_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct InstallFailure {
    pub skill: String,
    pub agent: String,
    pub error: String,
}

/// A listed skill enriched with lock metadata (serialized by `list --json`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedSkill {
    pub name: String,
    pub path: PathBuf,
    pub scope: String,
    pub agents: Vec<String>,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub source_type: Option<String>,
}

#[derive(Debug)]
pub struct RemoveOutcome {
    /// Installed names scanned (used by the no-args hint).
    pub installed: Vec<String>,
    /// Requested names (used by the no-match hint).
    pub requested: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Default)]
pub struct UpdateOutcome {
    pub global: bool,
    pub updated: usize,
    pub failed: usize,
    pub updated_names: Vec<String>,
    pub failures: Vec<String>,
}

// ============================ Private helpers ============================

fn lock_path(env: &Env, global: bool) -> PathBuf {
    if global {
        global_lock_path(&env.home)
    } else {
        local_lock_path(&env.cwd)
    }
}

fn write_lock(
    parsed: &Source,
    selected: &[Skill],
    successful: &[InstallSuccess],
    global: bool,
    env: &Env,
) -> Result<()> {
    let lock_path = lock_path(env, global);
    let mut lock = read_local_lock(&lock_path);
    lock.version = 1;

    let successful_names: HashSet<&str> = successful.iter().map(|s| s.name.as_str()).collect();
    for skill in selected {
        if !successful_names.contains(skill.name.as_str()) {
            continue;
        }
        let hash = compute_folder_hash(&skill.dir).unwrap_or_default();
        let (source, source_type, source_url, ref_, skill_path) = lock_fields(parsed);
        let mut entry = LockEntry::new(&source, &source_type, hash);
        entry.source_url = source_url;
        entry.r#ref = ref_;
        entry.skill_path = skill_path;
        lock.skills.insert(sanitize_name(&skill.name), entry);
    }
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_local_lock(&lock, &lock_path)
}

fn resolve_scope(req: &UpdateRequest, env: &Env) -> bool {
    if req.global && !req.project {
        return true;
    }
    if req.project || has_project_skills(env) {
        false
    } else {
        true
    }
}

fn has_project_skills(env: &Env) -> bool {
    if local_lock_path(&env.cwd).exists() {
        return true;
    }
    env.cwd.join(".agents/skills").exists()
}
