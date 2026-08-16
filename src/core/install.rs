//! Install orchestration: canonical dir, symlink/copy fallback, installed listing.
//!
//! Symlink mode writes to the canonical dir first, then symlinks into each agent dir;
//! copy mode writes directly to the agent dir. Copies skip metadata.json/.git/__pycache__/__pypackages__.
//! Not supported: Eve sub-agents (`agent/subagents/<name>/skills`) and remote/blob installs.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::agents::{AGENTS, Agent, Env, global_skills_dir, universal_agents};
use crate::core::discover::{Skill, parse_skill_md};
use crate::error::Result;

/// Install mode: symlink (default) or copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    /// Symlink the canonical dir into the agent dir.
    Symlink,
    /// Copy files directly into the agent dir.
    Copy,
}

/// Outcome of a single install attempt.
#[derive(Debug)]
pub struct InstallResult {
    /// Whether the install succeeded.
    pub success: bool,
    /// Destination path.
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Canonical directory (None in copy mode).
    pub canonical_path: Option<PathBuf>,
    /// Install mode used.
    #[allow(dead_code)]
    pub mode: InstallMode,
    /// Whether symlink creation failed.
    #[allow(dead_code)]
    pub symlink_failed: bool,
    /// Whether the install was skipped (already up to date).
    #[allow(dead_code)]
    pub skipped: bool,
    /// Error message on failure.
    pub error: Option<String>,
}

/// Sanitize a directory name: lowercase → fold non-`[a-z0-9._]` to `-` → trim leading/trailing `.`/`-` → truncate to 255 → fallback.
pub fn sanitize_name(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    // Collapse consecutive separators.
    let folded = {
        let mut out = String::new();
        let mut prev_dash = false;
        for c in s.chars() {
            if c == '-' {
                if !prev_dash {
                    out.push(c);
                }
                prev_dash = true;
            } else {
                out.push(c);
                prev_dash = false;
            }
        }
        out
    };
    s = folded;
    let trimmed = s.trim_matches(|c: char| c == '.' || c == '-').to_string();
    let mut result: String = trimmed.chars().take(255).collect();
    if result.is_empty() {
        result = "unnamed-skill".to_string();
    }
    result
}

/// Canonical skills dir: `(global ? home : cwd)/.agents/skills`.
pub fn canonical_skills_dir(global: bool, env: &Env) -> PathBuf {
    let base = if global { &env.home } else { &env.cwd };
    base.join(".agents").join("skills")
}

/// An agent's skills base dir (universal uses canonical; global uses global_skills_dir).
pub fn agent_base_dir(agent: &Agent, global: bool, env: &Env) -> Option<PathBuf> {
    if agent.is_universal() {
        return Some(canonical_skills_dir(global, env));
    }
    if global {
        global_skills_dir(agent, env)
    } else {
        Some(env.cwd.join(agent.skills_dir))
    }
}

/// Install path of a skill for an agent.
pub fn get_install_path(name: &str, agent: &Agent, global: bool, env: &Env) -> PathBuf {
    let sanitized = sanitize_name(name);
    agent_base_dir(agent, global, env)
        .unwrap_or_else(|| canonical_skills_dir(global, env))
        .join(sanitized)
}

/// Canonical path of a skill.
pub fn get_canonical_path(name: &str, global: bool, env: &Env) -> PathBuf {
    canonical_skills_dir(global, env).join(sanitize_name(name))
}

fn path_safe(base: &Path, target: &Path) -> bool {
    let base_abs = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let target_abs = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());
    target_abs == base_abs || target_abs.starts_with(&base_abs)
}

fn paths_overlap(a: &Path, b: &Path) -> bool {
    path_safe(a, b) || path_safe(b, a)
}

/// Clean and recreate a directory.
fn clean_and_create(dir: &Path) -> Result<()> {
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(dir)?;
    Ok(())
}

/// Recursively copy a directory, excluding metadata.json / .git / __pycache__ / __pypackages__,
/// dereferencing symlinks (copying target contents).
pub fn copy_directory(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let src_path = entry.path();
        let dest_path = dest.join(&name);

        let meta = entry.metadata()?;
        if meta.is_dir() {
            if name == ".git" || name == "__pycache__" || name == "__pypackages__" {
                continue;
            }
            copy_directory(&src_path, &dest_path)?;
        } else if meta.is_file() {
            if name == "metadata.json" {
                continue;
            }
            fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

/// Create a symlink; return false on failure (caller falls back to copy).
/// Windows uses a dir symlink; other platforms use a relative-path symlink.
fn create_symlink(target: &Path, link: &Path) -> bool {
    let target_abs = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());
    let link_abs = link.canonicalize().unwrap_or_else(|_| link.to_path_buf());
    if target_abs == link_abs {
        return true; // Same path, no symlink needed.
    }

    if let Some(parent) = link.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }
    let _ = fs::remove_file(link);
    let _ = fs::remove_dir_all(link);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target_abs, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(&target_abs, link).is_ok()
    }
}

/// Install a single skill to a single agent.
pub fn install_skill_for_agent(
    skill: &Skill,
    agent: &Agent,
    env: &Env,
    global: bool,
    mode: InstallMode,
) -> InstallResult {
    // global and the agent does not support global → fail.
    if global && agent_base_dir(agent, true, env).is_none() {
        return InstallResult {
            success: false,
            path: PathBuf::new(),
            canonical_path: None,
            mode,
            symlink_failed: false,
            skipped: false,
            error: Some(format!(
                "{} does not support global skill installation",
                agent.display
            )),
        };
    }

    let skill_name = sanitize_name(&skill.name);
    let canonical_base = canonical_skills_dir(global, env);
    let canonical_dir = canonical_base.join(&skill_name);
    let agent_base = agent_base_dir(agent, global, env).unwrap_or_else(|| canonical_base.clone());
    let agent_dir = agent_base.join(&skill_name);

    if !path_safe(&canonical_base, &canonical_dir) || !path_safe(&agent_base, &agent_dir) {
        return InstallResult {
            success: false,
            path: agent_dir,
            canonical_path: None,
            mode,
            symlink_failed: false,
            skipped: false,
            error: Some("Invalid skill name: potential path traversal detected".to_string()),
        };
    }

    // Source and target overlap → skip (avoid deleting the source).
    if paths_overlap(&skill.dir, &agent_dir) {
        return InstallResult {
            success: true,
            path: agent_dir,
            canonical_path: None,
            mode,
            symlink_failed: false,
            skipped: true,
            error: None,
        };
    }

    // copy mode: write directly to the agent dir.
    if mode == InstallMode::Copy {
        if let Err(e) =
            clean_and_create(&agent_dir).and_then(|_| copy_directory(&skill.dir, &agent_dir))
        {
            return InstallResult {
                success: false,
                path: agent_dir,
                canonical_path: None,
                mode,
                symlink_failed: false,
                skipped: false,
                error: Some(e.to_string()),
            };
        }
        return InstallResult {
            success: true,
            path: agent_dir,
            canonical_path: None,
            mode,
            symlink_failed: false,
            skipped: false,
            error: None,
        };
    }

    // symlink mode: write canonical first.
    if paths_overlap(&skill.dir, &canonical_dir) {
        return InstallResult {
            success: true,
            path: canonical_dir.clone(),
            canonical_path: Some(canonical_dir),
            mode,
            symlink_failed: false,
            skipped: true,
            error: None,
        };
    }

    if let Err(e) =
        clean_and_create(&canonical_dir).and_then(|_| copy_directory(&skill.dir, &canonical_dir))
    {
        return InstallResult {
            success: false,
            path: canonical_dir.clone(),
            canonical_path: Some(canonical_dir),
            mode,
            symlink_failed: false,
            skipped: false,
            error: Some(e.to_string()),
        };
    }

    // global + universal: canonical is the target, no symlink needed.
    if global && agent.is_universal() {
        return InstallResult {
            success: true,
            path: canonical_dir.clone(),
            canonical_path: Some(canonical_dir),
            mode,
            symlink_failed: false,
            skipped: false,
            error: None,
        };
    }

    // project-level + non-universal: agent root missing and not claude-code → skip symlink.
    if !global && !agent.is_universal() {
        let agent_root = env
            .cwd
            .join(agent.skills_dir.split('/').next().unwrap_or(""));
        if !agent_root.exists() && agent.name != "claude-code" {
            return InstallResult {
                success: true,
                path: canonical_dir.clone(),
                canonical_path: Some(canonical_dir),
                mode,
                symlink_failed: false,
                skipped: true,
                error: None,
            };
        }
    }

    let symlink_created = create_symlink(&canonical_dir, &agent_dir);
    if !symlink_created {
        // Fall back to copy.
        if let Err(e) =
            clean_and_create(&agent_dir).and_then(|_| copy_directory(&skill.dir, &agent_dir))
        {
            return InstallResult {
                success: false,
                path: agent_dir,
                canonical_path: Some(canonical_dir),
                mode,
                symlink_failed: true,
                skipped: false,
                error: Some(e.to_string()),
            };
        }
        return InstallResult {
            success: true,
            path: agent_dir,
            canonical_path: Some(canonical_dir),
            mode,
            symlink_failed: true,
            skipped: false,
            error: None,
        };
    }

    InstallResult {
        success: true,
        path: agent_dir,
        canonical_path: Some(canonical_dir),
        mode,
        symlink_failed: false,
        skipped: false,
        error: None,
    }
}

/// An installed skill (used by list).
#[derive(Debug)]
pub struct InstalledSkill {
    /// Skill name.
    pub name: String,
    /// Skill description.
    #[allow(dead_code)]
    pub description: String,
    /// Canonical directory path.
    pub canonical_path: PathBuf,
    /// `"project"` or `"global"`.
    pub scope: String,
    /// Agent names this skill is linked to.
    pub agents: Vec<String>,
}

/// Scan the canonical dir, listing installed skills (including per-agent ownership).
pub fn list_installed_skills(
    env: &Env,
    global: bool,
    agent_filter: &[String],
) -> Vec<InstalledSkill> {
    let scope = if global { "global" } else { "project" };
    let canonical = canonical_skills_dir(global, env);
    let mut out: Vec<InstalledSkill> = Vec::new();

    let entries = match fs::read_dir(&canonical) {
        Ok(e) => e,
        Err(_) => return out,
    };

    for entry in entries.flatten() {
        let skill_dir = entry.path();
        let skill_md = skill_dir.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let Some(skill) = parse_skill_md(&skill_md) else {
            continue;
        };
        let mut agents: Vec<String> = Vec::new();
        // Which agents own it (universal owns directly; non-universal checked by dir existence).
        for agent in universal_agents() {
            if !agent_filter.is_empty() && !agent_filter.iter().any(|a| a == agent.name) {
                continue;
            }
            agents.push(agent.name.to_string());
        }
        for agent in crate::core::agents::AGENTS
            .iter()
            .filter(|a| !a.is_universal())
        {
            if !agent_filter.is_empty() && !agent_filter.iter().any(|a| a == agent.name) {
                continue;
            }
            let Some(base) = agent_base_dir(agent, global, env) else {
                continue;
            };
            let candidate = base.join(sanitize_name(&skill.name));
            if candidate.exists() || base.join(entry.file_name()).exists() {
                agents.push(agent.name.to_string());
            }
        }
        out.push(InstalledSkill {
            name: skill.name,
            description: skill.description,
            canonical_path: skill_dir,
            scope: scope.to_string(),
            agents,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Match a discovered skill by sanitized name or skillPath directory name.
pub fn find_skill<'a>(
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
    if let Some(sp) = skill_path {
        let dir_name = sp.split('/').rfind(|p| !p.is_empty());
        if let Some(dn) = dir_name
            && let Some(s) = discovered.iter().find(|s| {
                s.dir
                    .file_name()
                    .map(|f| f.to_string_lossy() == dn)
                    .unwrap_or(false)
            })
        {
            return Some(s);
        }
    }
    discovered.first().filter(|_| discovered.len() == 1)
}

/// Whether a skill name matches a case-insensitive filter (empty filter matches all).
pub fn matches_skill(name: &str, filter: &[String]) -> bool {
    if filter.is_empty() {
        return true;
    }
    let lower = name.to_lowercase();
    filter.iter().any(|f| f.to_lowercase() == lower)
}

/// Scan the canonical dir + each agent dir, collecting installed skill directory names.
pub fn scan_installed(env: &Env, global: bool) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    let canonical = get_canonical_path("", global, env);
    let base = canonical.parent().map(PathBuf::from).unwrap_or_default();
    collect_dir_names(&base, &mut set);
    for agent in AGENTS {
        if let Some(dir) = agent_base_dir(agent, global, env) {
            collect_dir_names(&dir, &mut set);
        }
    }
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v
}

fn collect_dir_names(dir: &Path, set: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            set.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
}

/// Resolve skill names to remove: match by sanitized name, lock keys take priority.
pub fn resolve_to_remove(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agents::get_agent;
    use crate::core::test_utils::{env_at, write_and_parse_skill};

    fn write_skill(dir: &Path, name: &str) -> Skill {
        write_and_parse_skill(dir, name)
    }

    #[test]
    fn sanitize_name_basic() {
        assert_eq!(sanitize_name("PDF Master"), "pdf-master");
        assert_eq!(
            sanitize_name("Git Review Before Commit"),
            "git-review-before-commit"
        );
        assert_eq!(sanitize_name("../evil"), "evil");
        assert_eq!(sanitize_name("  "), "unnamed-skill");
        assert_eq!(sanitize_name("A.B_c"), "a.b_c");
        assert_eq!(sanitize_name("-leading-trailing-"), "leading-trailing");
    }

    #[test]
    fn canonical_dir_project_and_global() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        assert_eq!(
            canonical_skills_dir(false, &env),
            tmp.path().join(".agents/skills")
        );
        assert_eq!(
            canonical_skills_dir(true, &env),
            tmp.path().join(".agents/skills")
        );
    }

    #[test]
    fn install_to_universal_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let src = tmp.path().join("src-skill");
        let skill = write_skill(&src, "pdf");
        let agent = get_agent("amp").unwrap(); // universal

        let r = install_skill_for_agent(&skill, agent, &env, false, InstallMode::Symlink);
        assert!(r.success, "err={:?}", r.error);
        assert!(tmp.path().join(".agents/skills/pdf/SKILL.md").exists());
    }

    #[test]
    fn install_non_universal_project_skips_when_agent_dir_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let src = tmp.path().join("src-skill");
        let skill = write_skill(&src, "pdf");
        let agent = get_agent("windsurf").unwrap(); // non-universal, dir missing

        let r = install_skill_for_agent(&skill, agent, &env, false, InstallMode::Symlink);
        assert!(r.success);
        assert!(r.skipped);
        // canonical is still written.
        assert!(tmp.path().join(".agents/skills/pdf/SKILL.md").exists());
        assert!(!tmp.path().join(".windsurf/skills/pdf").exists());
    }

    #[test]
    fn install_claude_code_creates_symlink_even_when_dir_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let src = tmp.path().join("src-skill");
        let skill = write_skill(&src, "pdf");
        let agent = get_agent("claude-code").unwrap();

        let r = install_skill_for_agent(&skill, agent, &env, false, InstallMode::Symlink);
        assert!(r.success);
        assert!(r.canonical_path.is_some());
        assert!(tmp.path().join(".claude/skills/pdf").exists());
    }

    #[test]
    fn install_global_unsupported_agent_fails() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let src = tmp.path().join("src-skill");
        let skill = write_skill(&src, "pdf");
        let agent = get_agent("eve").unwrap(); // GlobalDir::None

        let r = install_skill_for_agent(&skill, agent, &env, true, InstallMode::Symlink);
        assert!(!r.success);
        assert!(r.error.unwrap().contains("does not support global"));
    }

    #[test]
    fn copy_mode_writes_directly_to_agent_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let src = tmp.path().join("src-skill");
        let skill = write_skill(&src, "pdf");
        let agent = get_agent("amp").unwrap();

        let r = install_skill_for_agent(&skill, agent, &env, false, InstallMode::Copy);
        assert!(r.success);
        assert!(tmp.path().join(".agents/skills/pdf/SKILL.md").exists());
    }

    #[test]
    fn copy_directory_excludes_ignored() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir_all(src.join(".git")).unwrap();
        fs::create_dir_all(src.join("__pycache__")).unwrap();
        fs::write(src.join("SKILL.md"), "x").unwrap();
        fs::write(src.join("metadata.json"), "x").unwrap();
        fs::write(src.join(".git").join("HEAD"), "x").unwrap();

        let dest = tmp.path().join("dest");
        copy_directory(&src, &dest).unwrap();
        assert!(dest.join("SKILL.md").exists());
        assert!(!dest.join("metadata.json").exists());
        assert!(!dest.join(".git").exists());
        assert!(!dest.join("__pycache__").exists());
    }

    #[test]
    fn list_installed_skills_finds_canonical() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let src = tmp.path().join("src-skill");
        let skill = write_skill(&src, "pdf");
        install_skill_for_agent(
            &skill,
            get_agent("amp").unwrap(),
            &env,
            false,
            InstallMode::Symlink,
        );

        let installed = list_installed_skills(&env, false, &[]);
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "pdf");
        assert_eq!(installed[0].scope, "project");
    }
}
