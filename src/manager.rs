//! High-level `Manager` facade: one-stop add/list/remove/update over an injectable [`Env`].
//!
//! The manager is pure data: it returns structured outcomes and never prints or exits;
//! the CLI layer (src/commands) is responsible for rendering.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use serde::Serialize;

use crate::core::agents::{
    AGENTS, Agent, Env, agent_display, config_home, detect_installed_agents,
    ensure_universal_agents, get_agent, home, is_installed,
};
use crate::core::discover::{Skill, discover_skills, filter_skills};
use crate::core::fetch::{clone_repo, download_and_extract};
use crate::core::install::{
    get_canonical_path, install_skill, list_installed_skills, sanitize_name, scan_installed,
};
use crate::core::link::{LinkOutcome, is_agent_linked, link_agent, unlink_agent};
use crate::core::lock::{
    LockEntry, compute_folder_hash, find_lock_entry, global_lock_path, local_lock_path,
    lock_fields, read_local_lock, write_local_lock,
};
use crate::core::source::{Source, SourceType, parse_source};
use crate::error::{Result, SkillsError};

/// Skill manager: carries injectable context and runs add/list/remove/update.
///
/// This is the high-level entry point for library consumers. It resolves an [`Env`]
/// (home / config / cwd) once at construction, then every operation is a plain method
/// taking a request struct and returning a structured outcome.
///
/// # Examples
///
/// ```
/// use agents_skills::{AddRequest, Manager};
///
/// // Real environment:
/// let real = Manager::new();
///
/// // Or a sandboxed environment (no side effects outside the given paths):
/// let sandboxed = Manager::builder()
///     .home("/tmp/home")
///     .config("/tmp/config")
///     .cwd("/tmp/project")
///     .build();
///
/// let req = AddRequest::new("anthropics/skills");
/// let _ = (real, sandboxed, req);
/// ```
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
    ///
    /// Equivalent to [`Manager::builder`]`().build()`.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Start customizing a manager (inject home/config/cwd/env vars).
    ///
    /// # Examples
    ///
    /// ```
    /// use agents_skills::Manager;
    ///
    /// let manager = Manager::builder()
    ///     .home("/tmp/home")
    ///     .env_var("CLAUDE_CONFIG_DIR", "/tmp/claude")
    ///     .build();
    /// ```
    pub fn builder() -> ManagerBuilder {
        ManagerBuilder::default()
    }

    /// Access the resolved environment context.
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Add (install) skills from a source.
    ///
    /// Parses the source, discovers its skills, and installs each selected skill
    /// into the canonical dir (the only place real files live), recording
    /// successful installs in the lockfile. Returns a structured [`AddOutcome`]
    /// with discovered, selected, installed and failed skills.
    ///
    /// `add` never links any agent: use [`Manager::link`] to expose the canonical
    /// dir to an agent afterwards.
    ///
    /// # Selection defaults
    ///
    /// - `skills` empty → all discovered skills; a `"*"` entry → all as well.
    /// - `list_only` → discover and report, without installing anything.
    ///
    /// # Examples
    ///
    /// Install a local skill into a scratch environment (hermetic — no network, no
    /// real home access):
    ///
    /// ```
    /// use agents_skills::{AddRequest, Manager};
    ///
    /// let tmp = tempfile::TempDir::new().unwrap();
    /// let src = tmp.path().join("hello");
    /// std::fs::create_dir_all(&src).unwrap();
    /// std::fs::write(
    ///     src.join("SKILL.md"),
    ///     "---\nname: hello\ndescription: says hello\n---\n\n# hello\n",
    /// )
    /// .unwrap();
    ///
    /// let manager = Manager::builder()
    ///     .home(tmp.path().join("home"))
    ///     .config(tmp.path().join("config"))
    ///     .cwd(tmp.path().join("project"))
    ///     .build();
    ///
    /// let outcome = manager.add(&AddRequest::new(src.display().to_string()))?;
    /// assert!(!outcome.installed.is_empty());
    /// # Ok::<(), agents_skills::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// - [`SkillsError::Message`] when the source is invalid, unreadable, or contains
    ///   no valid skill (a `SKILL.md` with `name` and `description`).
    /// - [`SkillsError::Git`], [`SkillsError::Http`], [`SkillsError::Io`],
    ///   [`SkillsError::Zip`], etc. for transport and filesystem failures.
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

        // Install into the canonical dir (the only place real files live).
        let mut installed: Vec<InstallSuccess> = Vec::new();
        let mut failed: Vec<InstallFailure> = Vec::new();
        for skill in &selected {
            let r = install_skill(skill, req.global, &self.env);
            if r.success && !r.skipped {
                installed.push(InstallSuccess {
                    name: skill.name.clone(),
                    canonical_path: r.canonical_path,
                });
            } else if !r.success {
                failed.push(InstallFailure {
                    skill: skill.name.clone(),
                    error: r.error.unwrap_or_default(),
                });
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
            installed,
            failed,
            list_only: false,
        })
    }

    /// Convenience: install every skill from `source` with default options.
    ///
    /// Equivalent to [`Manager::add`] with an [`AddRequest`] containing only a `source`
    /// (and therefore default selection: project scope, all skills).
    ///
    /// # Examples
    ///
    /// ```
    /// use agents_skills::Manager;
    ///
    /// let manager = Manager::new();
    /// // `add_source` with a bad source yields a structured error (no panic).
    /// let result = manager.add_source("./does-not-exist");
    /// assert!(result.is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Same as [`Manager::add`]: see its `# Errors` section.
    pub fn add_source(&self, source: impl Into<String>) -> Result<AddOutcome> {
        self.add(&AddRequest::new(source))
    }

    /// Link agents' skills dirs to the canonical dir.
    ///
    /// Each target agent's own skills dir becomes a relative symlink pointing at the
    /// canonical dir, so every skill installed there is immediately visible to that
    /// agent. Universal agents (whose skills dir already is the canonical dir) are
    /// reported as [`LinkOutcome::AlreadyLinked`]. Agents whose root dir does not
    /// exist in this scope are reported as [`LinkOutcome::Skipped`] (except
    /// `claude-code`, the historical exception).
    ///
    /// An agent dir that already holds content is refused; pass `migrate` to move its
    /// skill subdirs into the canonical dir first. Skills whose name already exists in
    /// the canonical dir are skipped (the canonical copy wins, reported via
    /// [`LinkOutcome::Migrated`] `skipped`); non-skill entries abort the migration
    /// without changes. Legacy per-skill symlinks pointing into the canonical dir are
    /// taken over automatically.
    ///
    /// # Selection defaults
    ///
    /// - `agents` empty → auto-detect installed agents (plus the universal agents);
    ///   a `"*"` entry → every known agent.
    ///
    /// # Errors
    ///
    /// [`SkillsError::InvalidAgents`] when `agents` names an unknown agent.
    /// Link or unlink agents' skills dirs relative to the canonical dir.
    ///
    /// Connects each agent's own skills dir to the canonical dir with a
    /// directory-level symlink, so every install/update/remove is immediately
    /// visible to all linked agents. With `req.unlink`, disconnects those dirs
    /// instead — removes the symlink (only when it points at the canonical dir)
    /// and recreates an empty real dir; the canonical dir and its skills are
    /// left untouched.
    ///
    /// When linking, an agent dir that already holds content is refused; pass
    /// `req.migrate` to move its skill subdirs into the canonical dir first.
    /// Skills whose name already exists in the canonical dir are skipped (the
    /// canonical copy wins); non-skill entries abort the migration without
    /// changes. Legacy per-skill symlinks pointing into the canonical dir are
    /// taken over automatically.
    ///
    /// # Selection defaults
    ///
    /// - `agents` empty → auto-detect installed agents (plus the universal agents);
    ///   a `"*"` entry → every known agent.
    ///
    /// # Errors
    ///
    /// [`SkillsError::InvalidAgents`] when `agents` names an unknown agent.
    pub fn link(&self, req: &LinkRequest) -> Result<LinkManagerOutcome> {
        let target_agents = resolve_target_agents(&req.agents, &self.env)?;
        let results = target_agents
            .iter()
            .map(|agent| AgentLinkResult {
                agent: agent.name.to_string(),
                display: agent.display.to_string(),
                outcome: if req.unlink {
                    unlink_agent(agent, req.global, &self.env)
                } else {
                    link_agent(agent, req.global, &self.env, req.migrate)
                },
            })
            .collect();
        Ok(LinkManagerOutcome {
            global: req.global,
            results,
        })
    }

    /// Link status of every installed agent in this scope.
    ///
    /// Only agents detected as installed locally (or already linked) are reported.
    /// Agents that natively read the canonical dir (universal) report `canonical`;
    /// agents connected via a directory-level symlink report `linked`.
    pub fn link_status(&self, global: bool) -> Vec<LinkStatus> {
        AGENTS
            .iter()
            .filter(|a| {
                is_installed(a, &self.env)
                    || (!a.is_universal() && is_agent_linked(a, global, &self.env))
            })
            .map(|a| LinkStatus {
                name: a.name.to_string(),
                display: a.display.to_string(),
                linked: is_agent_linked(a, global, &self.env),
                canonical: a.is_universal(),
            })
            .collect()
    }

    /// List installed skills (project or global), enriched with lock metadata.
    ///
    /// Scans the canonical skills directory and joins each entry with its lockfile
    /// record, producing serde-serializable [`ListedSkill`] values — the same shape
    /// emitted by `list --json`.
    ///
    /// # Examples
    ///
    /// ```
    /// use agents_skills::{ListRequest, Manager};
    ///
    /// let manager = Manager::new();
    /// let skills = manager.list(&ListRequest::default())?;
    /// for skill in skills {
    ///     println!("{} -> {}", skill.name, skill.path.display());
    /// }
    /// # Ok::<(), agents_skills::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`SkillsError::InvalidAgents`] when `agents` names an unknown agent.
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
    ///
    /// Deletes each skill's directory from the canonical dir and drops its lockfile
    /// entry. Removal applies to every linked agent at once (they all share the
    /// canonical dir); agent links themselves are untouched — call [`Manager::link`]
    /// with `unlink: true` to disconnect an agent instead.
    ///
    /// # Selection semantics
    ///
    /// - `skills` empty and `all` false → nothing is removed; the outcome reports the
    ///   currently installed names (used by the CLI to print a hint).
    /// - `all` true → every installed skill plus every lockfile key.
    ///
    /// # Examples
    ///
    /// ```
    /// use agents_skills::{Manager, RemoveRequest};
    ///
    /// let tmp = tempfile::TempDir::new().unwrap();
    /// let manager = Manager::builder()
    ///     .home(tmp.path().join("home"))
    ///     .cwd(tmp.path().join("project"))
    ///     .build();
    ///
    /// let req = RemoveRequest {
    ///     skills: vec!["pdf".to_string()],
    ///     ..Default::default()
    /// };
    /// // Nothing installed in the scratch dir, so this is a harmless no-op.
    /// let outcome = manager.remove(&req)?;
    /// assert!(outcome.removed.is_empty());
    /// # Ok::<(), agents_skills::Error>(())
    /// ```
    pub fn remove(&self, req: &RemoveRequest) -> Result<RemoveOutcome> {
        let global = req.global;

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
        let requested: Vec<String> = if req.all {
            installed.iter().chain(lock_keys.iter()).cloned().collect()
        } else {
            req.skills.clone()
        };
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

        // Remove from the canonical dir (visible to every linked agent at once).
        let mut removed: Vec<String> = Vec::new();
        for name in &selected {
            let canonical = get_canonical_path(name, global, &self.env);
            let sanitized = sanitize_name(name);
            let _ = std::fs::remove_dir_all(&canonical);

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
    ///
    /// Reads the lockfile, re-clones each recorded source once (skills sharing a source
    /// are grouped), re-installs the latest version into the canonical dir (all linked
    /// agents see the update immediately), and reports per-skill success/failure
    /// counts. Locally-sourced skills are skipped.
    ///
    /// # Scope resolution
    ///
    /// [`UpdateRequest::scope`] is [`Scope::Auto`] by default: project scope if the
    /// project has skills or a lockfile, otherwise global.
    ///
    /// # Examples
    ///
    /// ```
    /// use agents_skills::{Manager, UpdateRequest};
    ///
    /// let tmp = tempfile::TempDir::new().unwrap();
    /// let manager = Manager::builder()
    ///     .home(tmp.path().join("home"))
    ///     .cwd(tmp.path().join("project"))
    ///     .build();
    ///
    /// // No lockfile in the scratch dir, so nothing to update.
    /// let outcome = manager.update(&UpdateRequest::default())?;
    /// assert_eq!(outcome.updated, 0);
    /// # Ok::<(), agents_skills::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`SkillsError::Message`] when a recorded source fails to re-parse. Per-skill
    /// clone/install failures are captured in [`UpdateOutcome::failures`] rather than
    /// returned as errors.
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
                let r = install_skill(skill, global, &self.env);
                if r.success {
                    outcome.updated += 1;
                } else {
                    outcome.failed += 1;
                }
                outcome.updated_names.push(name.clone());
            }
        }

        Ok(outcome)
    }
}

/// Chained builder for [`Manager`], injecting home/config/cwd/env vars.
///
/// Every field is optional: unset fields fall back to the real environment at
/// [`build`](Self::build) time, so tests and sandboxes can override just the pieces
/// they care about.
#[derive(Default)]
pub struct ManagerBuilder {
    home: Option<PathBuf>,
    config: Option<PathBuf>,
    cwd: Option<PathBuf>,
    vars: std::collections::HashMap<String, String>,
}

impl ManagerBuilder {
    /// Override the home directory.
    ///
    /// Affects global skills (`~/.agents/skills`), the global lockfile, and per-agent
    /// user-level skills directories.
    pub fn home(mut self, p: impl Into<PathBuf>) -> Self {
        self.home = Some(p.into());
        self
    }

    /// Override the config directory.
    ///
    /// Affects agent config lookup (e.g. `CLAUDE_CONFIG_DIR` resolution).
    pub fn config(mut self, p: impl Into<PathBuf>) -> Self {
        self.config = Some(p.into());
        self
    }

    /// Override the current working directory.
    ///
    /// Affects project-scope installs (`.agents/skills`), the project lockfile, and
    /// scope auto-detection.
    pub fn cwd(mut self, p: impl Into<PathBuf>) -> Self {
        self.cwd = Some(p.into());
        self
    }

    /// Inject an environment variable override.
    ///
    /// Useful for redirecting agent-specific env vars (e.g. `CLAUDE_CONFIG_DIR`) that
    /// the agent directory mapping consults. Does not touch the real process env.
    pub fn env_var(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.vars.insert(k.into(), v.into());
        self
    }

    /// Build the [`Manager`], resolving defaults from the real environment.
    ///
    /// Unset fields fall back to the actual home/config/cwd of the process.
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

/// Request for [`Manager::add`].
///
/// The struct is `Default + Clone`; use [`AddRequest::new`] for the common
/// "install everything from a source" case and struct-update syntax
/// (`..Default::default()`) to override just the fields you need.
///
/// See the crate-level [source formats](crate#source-formats) table for the accepted
/// `source` strings.
#[derive(Debug, Clone, Default)]
pub struct AddRequest {
    /// Source string (local path, GitHub `owner/repo`, git URL, or download URL).
    pub source: String,
    /// Install globally (user-level, `~/.agents/skills`) instead of project-level.
    pub global: bool,
    /// `"*"` or specific skill names; empty = all discovered skills.
    pub skills: Vec<String>,
    /// List available skills without installing anything.
    pub list_only: bool,
    /// Recurse into nested skill directories beyond the default container depth.
    pub full_depth: bool,
}

impl AddRequest {
    /// Create a request that installs all skills from `source` with default options.
    ///
    /// All other fields default: project scope, all skills.
    ///
    /// # Examples
    ///
    /// ```
    /// use agents_skills::{AddRequest, Manager};
    ///
    /// let req = AddRequest::new("anthropics/skills");
    /// assert_eq!(req.source, "anthropics/skills");
    /// assert!(req.skills.is_empty()); // all discovered skills
    /// # let _ = Manager::new();
    /// ```
    pub fn new(source: impl Into<String>) -> Self {
        AddRequest {
            source: source.into(),
            ..Default::default()
        }
    }
}

/// Request for [`Manager::list`].
///
/// `Default` lists project-scope skills across all agents.
#[derive(Debug, Clone, Default)]
pub struct ListRequest {
    /// List global skills instead of project skills.
    pub global: bool,
    /// Filter by agent names; empty = all agents.
    pub agents: Vec<String>,
}

/// Request for [`Manager::remove`].
///
/// `Default` is a no-op that only reports installed names — set `skills` or `all` to
/// actually remove anything.
#[derive(Debug, Clone, Default)]
pub struct RemoveRequest {
    /// Skill names to remove (the CLI merges positional args and `--skill` here).
    pub skills: Vec<String>,
    /// Remove global skills instead of project skills.
    pub global: bool,
    /// Remove all installed skills.
    pub all: bool,
}

/// Request for [`Manager::link`] — one entry point mirroring the `link` CLI command.
///
/// `Default` links the auto-detected installed agents at project scope.
#[derive(Debug, Clone, Default)]
pub struct LinkRequest {
    /// `"*"` or specific agent names; empty = auto-detect installed agents.
    pub agents: Vec<String>,
    /// Link global skills dirs instead of project ones.
    pub global: bool,
    /// Unlink (disconnect) the agents' skills dirs instead of linking them.
    pub unlink: bool,
    /// Move existing agent skills dirs into the canonical dir before linking.
    pub migrate: bool,
}

/// Installation scope for [`Manager::update`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    /// Auto-detect: project scope if the project has skills or a lockfile, otherwise
    /// global.
    #[default]
    Auto,
    /// Force global scope.
    Global,
    /// Force project scope.
    Project,
}

/// Request for [`Manager::update`].
///
/// `Default` updates all skills with auto-detected scope.
#[derive(Debug, Clone, Default)]
pub struct UpdateRequest {
    /// Filter by skill names; empty = all.
    pub skills: Vec<String>,
    /// Installation scope ([`Scope::Auto`] by default).
    pub scope: Scope,
}

// ============================ Outcome types ============================

/// Result of [`Manager::add`].
///
/// Carries the full picture of an add operation: what was discovered, what was
/// selected, and which skills were installed into the canonical dir.
#[derive(Debug)]
pub struct AddOutcome {
    /// The parsed source.
    pub source: Source,
    /// All discovered skills.
    pub skills: Vec<Skill>,
    /// Selected skills (empty when `list_only`).
    pub selected: Vec<Skill>,
    /// Successfully installed skills.
    pub installed: Vec<InstallSuccess>,
    /// Failed installations.
    pub failed: Vec<InstallFailure>,
    /// Whether this was a `--list` request.
    pub list_only: bool,
}

/// A single successful install (one skill, into the canonical dir).
#[derive(Debug)]
pub struct InstallSuccess {
    /// Skill name.
    pub name: String,
    /// Canonical directory.
    pub canonical_path: PathBuf,
}

/// A single failed install.
#[derive(Debug)]
pub struct InstallFailure {
    /// Skill name.
    pub skill: String,
    /// Error message.
    pub error: String,
}

/// Result of linking (or unlinking) one agent's skills dir relative to the
/// canonical dir.
#[derive(Debug)]
pub struct AgentLinkResult {
    /// Agent identifier (as used on the CLI).
    pub agent: String,
    /// Agent display name.
    pub display: String,
    /// Link/unlink outcome details.
    pub outcome: LinkOutcome,
}

/// Link status of one agent (used by `link --status`).
#[derive(Debug)]
pub struct LinkStatus {
    /// Agent identifier (as used on the CLI).
    pub name: String,
    /// Agent display name.
    pub display: String,
    /// Whether the agent's skills dir is linked to the canonical dir.
    pub linked: bool,
    /// Whether the agent natively uses the canonical dir (no link involved).
    pub canonical: bool,
}

/// Result of [`Manager::link`].
#[derive(Debug)]
pub struct LinkManagerOutcome {
    /// Whether the links used global scope.
    pub global: bool,
    /// Per-agent link results.
    pub results: Vec<AgentLinkResult>,
}

/// A listed skill enriched with lock metadata (serialized by `list --json`).
///
/// Fields are serialized in camelCase, so `source_url` becomes `"sourceUrl"` — the
/// exact JSON shape emitted by the CLI's `list --json`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedSkill {
    /// Skill name.
    pub name: String,
    /// Canonical directory path.
    pub path: PathBuf,
    /// `"project"` or `"global"`.
    pub scope: String,
    /// Agent display names this skill is linked to.
    pub agents: Vec<String>,
    /// Source identifier from the lock.
    pub source: Option<String>,
    /// Resolved source URL.
    pub source_url: Option<String>,
    /// Source type (e.g. `"github"`, `"local"`).
    pub source_type: Option<String>,
}

/// Result of [`Manager::remove`].
#[derive(Debug)]
pub struct RemoveOutcome {
    /// Installed names scanned (used by the no-args hint).
    pub installed: Vec<String>,
    /// Requested names (used by the no-match hint).
    pub requested: Vec<String>,
    /// Names actually removed.
    pub removed: Vec<String>,
}

/// Result of [`Manager::update`].
#[derive(Debug, Default)]
pub struct UpdateOutcome {
    /// Whether the update used global scope.
    pub global: bool,
    /// Number of successful updates (one count per skill).
    pub updated: usize,
    /// Number of failed updates.
    pub failed: usize,
    /// Names of skills that were updated.
    pub updated_names: Vec<String>,
    /// Human-readable failure messages.
    pub failures: Vec<String>,
}

// ============================ Private helpers ============================

/// Resolve agent selection: `"*"` → all; names → validated; empty → auto-detect + universal.
fn resolve_target_agents(names: &[String], env: &Env) -> Result<Vec<&'static Agent>> {
    if names.iter().any(|a| a == "*") {
        return Ok(AGENTS.iter().collect());
    }
    if !names.is_empty() {
        let mut agents = Vec::new();
        let mut invalid = Vec::new();
        for name in names {
            match get_agent(name) {
                Some(a) => agents.push(a),
                None => invalid.push(name.clone()),
            }
        }
        if !invalid.is_empty() {
            return Err(SkillsError::InvalidAgents(invalid.join(", ")));
        }
        return Ok(agents);
    }
    let installed = detect_installed_agents(env);
    Ok(ensure_universal_agents(installed))
}

/// Match a discovered skill by sanitized name or skillPath directory name.
fn find_skill<'a>(
    discovered: &'a [Skill],
    name: &str,
    skill_path: Option<&str>,
) -> Option<&'a Skill> {
    let sanitized = sanitize_name(name);
    // Prefer matching by (sanitized) name.
    if let Some(s) = discovered
        .iter()
        .find(|s| sanitize_name(&s.name) == sanitized)
    {
        return Some(s);
    }
    // Match by skillPath (directory name).
    if let Some(sp) = skill_path
        && let Some(dn) = sp.split('/').rfind(|p| !p.is_empty())
        && let Some(s) = discovered.iter().find(|s| {
            s.dir
                .file_name()
                .map(|f| f.to_string_lossy() == dn)
                .unwrap_or(false)
        })
    {
        return Some(s);
    }
    discovered.first().filter(|_| discovered.len() == 1)
}

/// Whether a skill name matches a case-insensitive filter (empty filter matches all).
fn matches_skill(name: &str, filter: &[String]) -> bool {
    if filter.is_empty() {
        return true;
    }
    let lower = name.to_lowercase();
    filter.iter().any(|f| f.to_lowercase() == lower)
}

/// Resolve skill names to remove: match by sanitized name, lock keys take priority.
fn resolve_to_remove(
    requested: &[String],
    installed: &[String],
    lock_keys: &[String],
) -> Vec<String> {
    let mut identity: HashMap<String, String> = HashMap::new();
    for folder in installed {
        identity
            .entry(sanitize_name(folder))
            .or_insert_with(|| folder.clone());
    }
    for key in lock_keys {
        identity.insert(sanitize_name(key), key.clone());
    }
    let mut matched = HashSet::new();
    for name in requested {
        if let Some(hit) = identity.get(&sanitize_name(name)) {
            matched.insert(hit.clone());
        }
    }
    let mut v: Vec<String> = matched.into_iter().collect();
    v.sort();
    v
}

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
    match req.scope {
        Scope::Global => true,
        Scope::Project => false,
        Scope::Auto => !has_project_skills(env),
    }
}

fn has_project_skills(env: &Env) -> bool {
    if local_lock_path(&env.cwd).exists() {
        return true;
    }
    env.cwd.join(".agents/skills").exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::test_utils::{env_at, write_and_parse_skill};

    fn skills_with_dirs(pairs: &[(&str, &str)]) -> Vec<Skill> {
        pairs
            .iter()
            .map(|(dir, name)| {
                let mut s = write_and_parse_skill(std::path::Path::new(dir), name);
                // write_and_parse_skill derives dir from the SKILL.md path; use the skill dir.
                s.dir = std::path::PathBuf::from(dir);
                s
            })
            .collect()
    }

    #[test]
    fn find_skill_prefers_name_then_skill_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("dir-a");
        let b = tmp.path().join("dir-b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let skills = skills_with_dirs(&[
            (a.to_str().unwrap(), "alpha"),
            (b.to_str().unwrap(), "beta"),
        ]);

        // By (sanitized) name.
        assert_eq!(find_skill(&skills, "Alpha", None).unwrap().name, "alpha");
        // By skillPath directory name.
        assert_eq!(
            find_skill(&skills, "missing", Some("x/dir-b"))
                .unwrap()
                .name,
            "beta"
        );
        // Ambiguous without a match.
        assert!(find_skill(&skills, "missing", None).is_none());
    }

    #[test]
    fn matches_skill_filters_case_insensitively() {
        let filter = vec!["PDF".to_string()];
        assert!(matches_skill("pdf", &filter));
        assert!(!matches_skill("git", &filter));
        assert!(matches_skill("anything", &[]));
    }

    #[test]
    fn resolve_to_remove_prefers_lock_keys() {
        let installed = vec!["PDF".to_string()];
        let lock_keys = vec!["pdf".to_string()];
        let requested = vec!["pdf".to_string(), "unknown".to_string()];

        // Lock keys take priority: "pdf" (not the on-disk "PDF" casing).
        assert_eq!(
            resolve_to_remove(&requested, &installed, &lock_keys),
            vec!["pdf"]
        );
    }

    #[test]
    fn resolve_target_agents_validates_names() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        assert!(resolve_target_agents(&["claude-code".to_string()], &env).is_ok());
        assert!(matches!(
            resolve_target_agents(&["nope".to_string()], &env),
            Err(SkillsError::InvalidAgents(_))
        ));
    }
}
