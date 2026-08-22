//! Directory-level agent links: connect each agent's skills dir to the canonical dir.
//!
//! The canonical dir holds the only real copies of installed skills; agents that do
//! not natively read it are integrated with a directory-level symlink
//! ([`link_agent`]): each agent's own skills dir becomes a relative link pointing
//! at the canonical dir, so every install/update/remove is instantly visible to all
//! linked agents. [`unlink_agent`] disconnects an agent without touching the skills
//! themselves.

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::agents::{Agent, Env, agent_skills_dir, canonical_skills_dir};

/// Outcome of linking one agent's skills dir to the canonical dir.
#[derive(Debug)]
pub enum LinkOutcome {
    /// A new directory-level symlink was created.
    Linked,
    /// The agent already uses the canonical dir (linked, or universal).
    AlreadyLinked,
    /// Existing content was moved into the canonical dir, then linked.
    Migrated {
        /// Names of the skill directories that were moved.
        moved: Vec<String>,
    },
    /// The agent's skills dir has content and `migrate` was not requested.
    Refused {
        /// Human-readable reason and remedy.
        reason: String,
    },
    /// The agent is not present in this scope (its root dir does not exist).
    Skipped,
    /// The link could not be established.
    Failed {
        /// Error message.
        error: String,
    },
}

/// Outcome of unlinking one agent's skills dir from the canonical dir.
#[derive(Debug)]
pub enum UnlinkOutcome {
    /// The symlink was removed and an empty real dir recreated.
    Unlinked,
    /// The agent's skills dir is not a link to the canonical dir (nothing to do).
    NotLinked,
    /// The unlink failed.
    Failed {
        /// Error message.
        error: String,
    },
}

/// Whether an agent's skills dir is linked to the canonical dir (universal = always).
pub fn is_agent_linked(agent: &Agent, global: bool, env: &Env) -> bool {
    if agent.is_universal() {
        return true;
    }
    match agent_skills_dir(agent, global, env) {
        Some(dir) => {
            fs::symlink_metadata(&dir)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
                && points_to(&dir, &canonical_skills_dir(global, env))
        }
        None => false,
    }
}

/// Link an agent's skills dir to the canonical dir (see [`LinkOutcome`] for cases).
///
/// Gating: non-universal agents are skipped when their root dir does not exist in the
/// given scope (project: first path component of `skills_dir`, e.g. `.windsurf`;
/// global: the parent of the agent's skills dir, e.g. `~/.claude`) — this avoids
/// fabricating agent presence. `claude-code` is the historical exception: it is
/// linked at project level even when `.claude/` does not exist yet.
pub fn link_agent(agent: &Agent, global: bool, env: &Env, migrate: bool) -> LinkOutcome {
    // Universal agents use the canonical dir natively — nothing to link.
    if agent.is_universal() {
        return LinkOutcome::AlreadyLinked;
    }

    let Some(agent_dir) = agent_skills_dir(agent, global, env) else {
        return LinkOutcome::Failed {
            error: format!("agent '{}' has no skills dir for this scope", agent.name),
        };
    };

    if !agent_root_exists(agent, global, env, &agent_dir) && agent.name != "claude-code" {
        return LinkOutcome::Skipped;
    }

    let canonical = canonical_skills_dir(global, env);

    match fs::symlink_metadata(&agent_dir) {
        // Missing: create the parent chain + a relative symlink.
        Err(_) => create_dir_symlink(&canonical, &agent_dir),
        Ok(meta) if meta.file_type().is_symlink() => {
            if points_to(&agent_dir, &canonical) {
                LinkOutcome::AlreadyLinked
            } else {
                LinkOutcome::Failed {
                    error: format!(
                        "{} is a symlink pointing elsewhere; remove it first",
                        agent_dir.display()
                    ),
                }
            }
        }
        Ok(_) => match fs::read_dir(&agent_dir) {
            Err(e) => LinkOutcome::Failed {
                error: e.to_string(),
            },
            Ok(entries) => {
                let entries: Vec<_> = entries.flatten().collect();
                if entries.is_empty() {
                    // Empty dir: safe to replace with the link.
                    let _ = fs::remove_dir(&agent_dir);
                    create_dir_symlink(&canonical, &agent_dir)
                } else if entries.iter().all(|e| is_legacy_link(e, &canonical)) {
                    // Old-model per-skill links into the canonical dir: take over.
                    let _ = fs::remove_dir_all(&agent_dir);
                    create_dir_symlink(&canonical, &agent_dir)
                } else if migrate {
                    migrate_and_link(&agent_dir, &canonical, &entries)
                } else {
                    LinkOutcome::Refused {
                        reason: format!(
                            "{} has existing content; rerun with --migrate to move it into {}",
                            agent_dir.display(),
                            canonical.display()
                        ),
                    }
                }
            }
        },
    }
}

/// Unlink an agent's skills dir from the canonical dir, recreating an empty real dir.
pub fn unlink_agent(agent: &Agent, global: bool, env: &Env) -> UnlinkOutcome {
    // Universal agents use the canonical dir natively — nothing to unlink.
    if agent.is_universal() {
        return UnlinkOutcome::NotLinked;
    }

    let Some(agent_dir) = agent_skills_dir(agent, global, env) else {
        return UnlinkOutcome::NotLinked;
    };

    let meta = match fs::symlink_metadata(&agent_dir) {
        Err(_) => return UnlinkOutcome::NotLinked,
        Ok(m) => m,
    };
    if !meta.file_type().is_symlink() {
        return UnlinkOutcome::NotLinked;
    }
    let canonical = canonical_skills_dir(global, env);
    if !points_to(&agent_dir, &canonical) {
        // A foreign symlink: leave it alone.
        return UnlinkOutcome::NotLinked;
    }

    if let Err(e) = fs::remove_file(&agent_dir) {
        return UnlinkOutcome::Failed {
            error: e.to_string(),
        };
    }
    // Recreate an empty dir so the agent does not see a missing skills dir.
    if let Err(e) = fs::create_dir_all(&agent_dir) {
        return UnlinkOutcome::Failed {
            error: e.to_string(),
        };
    }
    UnlinkOutcome::Unlinked
}

/// Whether the agent's root dir exists in this scope (project: first component of
/// `skills_dir`; global: parent of the agent's skills dir).
fn agent_root_exists(agent: &Agent, global: bool, env: &Env, agent_dir: &Path) -> bool {
    if global {
        agent_dir.parent().map(|p| p.exists()).unwrap_or(false)
    } else {
        let root = agent.skills_dir.split('/').next().unwrap_or("");
        env.cwd.join(root).exists()
    }
}

/// Whether `link` (a symlink) resolves to `target`.
fn points_to(link: &Path, target: &Path) -> bool {
    match fs::read_link(link) {
        Ok(raw) => {
            let resolved = if raw.is_absolute() {
                raw
            } else {
                link.parent().unwrap_or(Path::new(".")).join(raw)
            };
            same_path(&resolved, target)
        }
        Err(_) => false,
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => normalize_lexical(a) == normalize_lexical(b),
    }
}

/// Lexically resolve `.`/`..` components (no filesystem access).
fn normalize_lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Whether a dir entry is an old-model per-skill symlink into the canonical dir.
fn is_legacy_link(entry: &fs::DirEntry, canonical: &Path) -> bool {
    let path = entry.path();
    let is_symlink = fs::symlink_metadata(&path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    if !is_symlink {
        return false;
    }
    match fs::read_link(&path) {
        Ok(raw) => {
            let resolved = if raw.is_absolute() {
                raw
            } else {
                path.parent().unwrap_or(Path::new(".")).join(raw)
            };
            let canon = canonical
                .canonicalize()
                .unwrap_or_else(|_| normalize_lexical(canonical));
            let res = resolved
                .canonicalize()
                .unwrap_or_else(|_| normalize_lexical(&resolved));
            res == canon || res.starts_with(&canon)
        }
        Err(_) => false,
    }
}

/// Create `link` as a symlink to `canonical`, using a relative target when possible.
fn create_dir_symlink(canonical: &Path, link: &Path) -> LinkOutcome {
    if let Some(parent) = link.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return LinkOutcome::Failed {
            error: format!("create {}: {e}", parent.display()),
        };
    }
    let target = relative_target(canonical, link);
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(&target, link);
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_dir(&target, link);
    match result {
        Ok(()) => LinkOutcome::Linked,
        Err(e) => LinkOutcome::Failed {
            error: format!(
                "symlink {} -> {}: {e} (on Windows, enable Developer Mode to allow symlinks)",
                link.display(),
                target.display()
            ),
        },
    }
}

/// Relative path from `link`'s parent to `canonical` (absolute fallback).
fn relative_target(canonical: &Path, link: &Path) -> PathBuf {
    let base = link.parent().unwrap_or(Path::new("."));
    pathdiff::diff_paths(canonical, base).unwrap_or_else(|| canonical.to_path_buf())
}

/// Move every skill subdir of `agent_dir` into `canonical`, then link the dir.
///
/// All-or-nothing: name conflicts with existing canonical skills and non-skill
/// entries (stray files, foreign symlinks) abort before anything is moved.
fn migrate_and_link(agent_dir: &Path, canonical: &Path, entries: &[fs::DirEntry]) -> LinkOutcome {
    let mut moved: Vec<String> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();
    let mut strays: Vec<String> = Vec::new();

    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            return LinkOutcome::Failed {
                error: format!("stat {}: unreadable", path.display()),
            };
        };
        if meta.file_type().is_symlink() {
            if is_legacy_link(entry, canonical) {
                continue; // old-model artifact: content already lives in canonical
            }
            strays.push(name);
        } else if meta.is_dir() {
            if canonical.join(&name).exists() {
                conflicts.push(name);
            } else {
                moved.push(name);
            }
        } else {
            strays.push(name);
        }
    }

    if !conflicts.is_empty() || !strays.is_empty() {
        let mut reason = String::new();
        if !conflicts.is_empty() {
            reason.push_str(&format!(
                "name conflicts with canonical skills: {}",
                conflicts.join(", ")
            ));
        }
        if !strays.is_empty() {
            if !reason.is_empty() {
                reason.push_str("; ");
            }
            reason.push_str(&format!("non-skill entries: {}", strays.join(", ")));
        }
        return LinkOutcome::Failed {
            error: format!("cannot migrate {}: {reason}", agent_dir.display()),
        };
    }

    if let Err(e) = fs::create_dir_all(canonical) {
        return LinkOutcome::Failed {
            error: format!("create {}: {e}", canonical.display()),
        };
    }
    for name in &moved {
        let from = agent_dir.join(name);
        let to = canonical.join(name);
        if let Err(e) = fs::rename(&from, &to) {
            return LinkOutcome::Failed {
                error: format!("move {}: {e}", from.display()),
            };
        }
    }
    if let Err(e) = fs::remove_dir(agent_dir) {
        return LinkOutcome::Failed {
            error: format!("remove {}: {e}", agent_dir.display()),
        };
    }
    match create_dir_symlink(canonical, agent_dir) {
        LinkOutcome::Linked => LinkOutcome::Migrated { moved },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agents::get_agent;
    use crate::core::install::install_skill;
    use crate::core::test_utils::{env_at, write_and_parse_skill};

    /// Env with distinct home/cwd (for global-scope tests).
    fn split_env(tmp: &tempfile::TempDir) -> Env {
        Env::new(
            tmp.path().join("home"),
            tmp.path().join("config"),
            tmp.path().join("project"),
        )
    }

    #[test]
    fn link_agent_creates_relative_symlink_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".windsurf")).unwrap();
        let agent = get_agent("windsurf").unwrap();

        let outcome = link_agent(agent, false, &env, false);
        assert!(matches!(outcome, LinkOutcome::Linked));
        let link = tmp.path().join(".windsurf/skills");
        assert!(link.is_symlink());
        assert_eq!(
            fs::read_link(&link).unwrap(),
            Path::new("../.agents/skills")
        );
    }

    #[test]
    fn link_agent_global_links_home_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = split_env(&tmp);
        fs::create_dir_all(env.home.join(".claude/skills")).unwrap();
        let agent = get_agent("claude-code").unwrap();

        let outcome = link_agent(agent, true, &env, false);
        assert!(matches!(outcome, LinkOutcome::Linked));
        let link = env.home.join(".claude/skills");
        assert!(link.is_symlink());
        assert_eq!(
            fs::read_link(&link).unwrap(),
            Path::new("../.agents/skills")
        );
    }

    #[test]
    fn link_agent_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".windsurf")).unwrap();
        let agent = get_agent("windsurf").unwrap();

        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked
        ));
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::AlreadyLinked
        ));
    }

    #[test]
    fn link_agent_refuses_foreign_symlink() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".windsurf")).unwrap();
        fs::create_dir_all(tmp.path().join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(
            tmp.path().join("elsewhere"),
            tmp.path().join(".windsurf/skills"),
        )
        .unwrap();
        let agent = get_agent("windsurf").unwrap();

        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Failed { .. }
        ));
    }

    #[test]
    fn link_agent_skips_when_agent_root_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let agent = get_agent("windsurf").unwrap(); // .windsurf does not exist

        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Skipped
        ));
        assert!(!tmp.path().join(".windsurf").exists());
    }

    #[test]
    fn link_agent_claude_code_links_even_without_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let agent = get_agent("claude-code").unwrap(); // .claude does not exist

        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked
        ));
        assert!(tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_refuses_existing_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        fs::write(tmp.path().join(".claude/skills/my-skill/SKILL.md"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, false) {
            LinkOutcome::Refused { reason } => assert!(reason.contains("--migrate")),
            other => panic!("expected Refused, got {other:?}"),
        }
        // Content untouched.
        assert!(tmp.path().join(".claude/skills/my-skill/SKILL.md").exists());
    }

    #[test]
    fn link_agent_migrate_moves_content_and_links() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, true) {
            LinkOutcome::Migrated { moved } => assert_eq!(moved, vec!["my-skill"]),
            other => panic!("expected Migrated, got {other:?}"),
        }
        assert!(tmp.path().join(".agents/skills/my-skill/SKILL.md").exists());
        assert!(tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_migrate_conflict_aborts_without_changes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/pdf");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "agent copy").unwrap();
        // Same name already installed in canonical.
        fs::create_dir_all(tmp.path().join(".agents/skills/pdf")).unwrap();
        fs::write(tmp.path().join(".agents/skills/pdf/SKILL.md"), "canonical").unwrap();
        let agent = get_agent("claude-code").unwrap();

        assert!(matches!(
            link_agent(agent, false, &env, true),
            LinkOutcome::Failed { .. }
        ));
        // Nothing moved, both copies intact.
        assert_eq!(
            fs::read_to_string(existing.join("SKILL.md")).unwrap(),
            "agent copy"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join(".agents/skills/pdf/SKILL.md")).unwrap(),
            "canonical"
        );
    }

    #[test]
    fn link_agent_takes_over_legacy_per_skill_links() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        // Canonical already holds the skill (old model wrote it there).
        let src = tmp.path().join("src-skill");
        let skill = write_and_parse_skill(&src, "pdf");
        install_skill(&skill, false, &env);
        // Old-model agent dir: per-skill symlink pointing into canonical.
        fs::create_dir_all(tmp.path().join(".windsurf/skills")).unwrap();
        std::os::unix::fs::symlink(
            tmp.path().join(".agents/skills/pdf"),
            tmp.path().join(".windsurf/skills/pdf"),
        )
        .unwrap();
        let agent = get_agent("windsurf").unwrap();

        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked
        ));
        let link = tmp.path().join(".windsurf/skills");
        assert!(link.is_symlink());
        assert!(link.join("pdf/SKILL.md").exists());
    }

    #[test]
    fn unlink_agent_removes_link_and_recreates_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".windsurf")).unwrap();
        let agent = get_agent("windsurf").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked
        ));

        assert!(matches!(
            unlink_agent(agent, false, &env),
            UnlinkOutcome::Unlinked
        ));
        let dir = tmp.path().join(".windsurf/skills");
        assert!(dir.is_dir());
        assert!(!dir.is_symlink());
        assert!(fs::read_dir(&dir).unwrap().count() == 0);
    }

    #[test]
    fn unlink_agent_leaves_real_dirs_and_foreign_links_alone() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        let agent = get_agent("claude-code").unwrap();
        assert!(matches!(
            unlink_agent(agent, false, &env),
            UnlinkOutcome::NotLinked
        ));
        assert!(tmp.path().join(".claude/skills/my-skill").exists());

        // Universal agents never link.
        let uni = get_agent("amp").unwrap();
        assert!(matches!(
            unlink_agent(uni, false, &env),
            UnlinkOutcome::NotLinked
        ));
    }

    #[test]
    fn is_agent_linked_reflects_dir_links() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let agent = get_agent("windsurf").unwrap();
        assert!(!is_agent_linked(agent, false, &env));
        fs::create_dir_all(tmp.path().join(".windsurf")).unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked
        ));
        assert!(is_agent_linked(agent, false, &env));
        // Universal agents are always "linked" (canonical is their dir).
        assert!(is_agent_linked(get_agent("amp").unwrap(), false, &env));
    }
}
