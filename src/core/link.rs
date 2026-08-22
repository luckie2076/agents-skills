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
        /// Names of skills left in place because the canonical dir already has
        /// them (the canonical copies win).
        skipped: Vec<String>,
    },
    /// The agent's skills dir has content and `migrate` was not requested.
    Refused {
        /// Names of the skill directories already present in the agent's dir.
        skills: Vec<String>,
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
    /// The agent's skills dir was unlinked from the canonical dir; an empty real
    /// dir was recreated in its place.
    Unlinked,
    /// The agent's skills dir is not a link to the canonical dir (nothing to unlink).
    NotLinked,
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
                    let skills = skill_names(&entries, &canonical);
                    let strays: Vec<String> = entries
                        .iter()
                        .filter(|e| {
                            let path = e.path();
                            !is_legacy_link(e, &canonical)
                                && !fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false)
                        })
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect();
                    let reason = match (skills.is_empty(), strays.is_empty()) {
                        (false, true) => format!(
                            "{} has existing skills; rerun with --migrate to move them into {}",
                            agent_dir.display(),
                            canonical.display()
                        ),
                        (false, false) => format!(
                            "{} has existing skills and non-skill files; remove the files ({}), then rerun with --migrate",
                            agent_dir.display(),
                            strays.join(", ")
                        ),
                        (true, false) => format!(
                            "{} contains non-skill files; move them out and rerun (migrate only moves skill directories): {}",
                            agent_dir.display(),
                            strays.join(", ")
                        ),
                        (true, true) => format!(
                            "{} has existing content; rerun with --migrate to move it into {}",
                            agent_dir.display(),
                            canonical.display()
                        ),
                    };
                    LinkOutcome::Refused { skills, reason }
                }
            }
        },
    }
}

/// Unlink an agent's skills dir from the canonical dir, recreating an empty real dir.
///
/// Returns a [`LinkOutcome`]: [`LinkOutcome::Unlinked`] on success,
/// [`LinkOutcome::NotLinked`] when there is nothing to do.
pub fn unlink_agent(agent: &Agent, global: bool, env: &Env) -> LinkOutcome {
    // Universal agents use the canonical dir natively — nothing to unlink.
    if agent.is_universal() {
        return LinkOutcome::NotLinked;
    }

    let Some(agent_dir) = agent_skills_dir(agent, global, env) else {
        return LinkOutcome::NotLinked;
    };

    let meta = match fs::symlink_metadata(&agent_dir) {
        Err(_) => return LinkOutcome::NotLinked,
        Ok(m) => m,
    };
    if !meta.file_type().is_symlink() {
        return LinkOutcome::NotLinked;
    }
    let canonical = canonical_skills_dir(global, env);
    if !points_to(&agent_dir, &canonical) {
        // A foreign symlink: leave it alone.
        return LinkOutcome::NotLinked;
    }

    if let Err(e) = fs::remove_file(&agent_dir) {
        return LinkOutcome::Failed {
            error: e.to_string(),
        };
    }
    // Recreate an empty dir so the agent does not see a missing skills dir.
    if let Err(e) = fs::create_dir_all(&agent_dir) {
        return LinkOutcome::Failed {
            error: e.to_string(),
        };
    }
    LinkOutcome::Unlinked
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

/// Names of skill dirs among `entries`: real subdirs and symlinks whose target is
/// a directory (e.g. links into a skills hub). Files and legacy per-skill links
/// are excluded — the latter already point into the canonical dir.
fn skill_names(entries: &[fs::DirEntry], canonical: &Path) -> Vec<String> {
    entries
        .iter()
        .filter(|e| {
            let path = e.path();
            !is_legacy_link(e, canonical)
                && fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false)
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
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

/// Move every skill of `agent_dir` into `canonical`, then link the dir.
///
/// Skills are real subdirs and symlinks whose target is a directory (e.g. links
/// into a skills hub); the latter are moved as links, preserving their semantics.
/// Skills whose name already exists in the canonical dir are skipped — the
/// canonical copy wins — and reported in `skipped`. Non-skill entries (stray
/// files, symlinks to non-directories) abort before anything is moved.
fn migrate_and_link(agent_dir: &Path, canonical: &Path, entries: &[fs::DirEntry]) -> LinkOutcome {
    let mut moved: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
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
            // A symlink whose target is a directory is a skill (e.g. a link into
            // a skills hub); moving it preserves the link semantics.
            if fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false) {
                if canonical.join(&name).exists() {
                    skipped.push(name);
                } else {
                    moved.push(name);
                }
            } else {
                strays.push(name);
            }
        } else if meta.is_dir() {
            if canonical.join(&name).exists() {
                skipped.push(name);
            } else {
                moved.push(name);
            }
        } else {
            strays.push(name);
        }
    }

    if !strays.is_empty() {
        return LinkOutcome::Failed {
            error: format!(
                "cannot migrate {}: non-skill entries: {}",
                agent_dir.display(),
                strays.join(", ")
            ),
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
    // Skip copies (and any legacy links) stay behind in the agent dir; all of
    // them have their content in the canonical dir, so the whole dir is cleared.
    if let Err(e) = fs::remove_dir_all(agent_dir) {
        return LinkOutcome::Failed {
            error: format!("remove {}: {e}", agent_dir.display()),
        };
    }
    match create_dir_symlink(canonical, agent_dir) {
        LinkOutcome::Linked => LinkOutcome::Migrated { moved, skipped },
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
        // A symlinked skill pointing elsewhere (e.g. into a skills hub) also counts.
        fs::create_dir_all(tmp.path().join("hub/other-skill")).unwrap();
        std::os::unix::fs::symlink(
            tmp.path().join("hub/other-skill"),
            tmp.path().join(".claude/skills/other-skill"),
        )
        .unwrap();
        // A stray file is not a skill.
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, false) {
            LinkOutcome::Refused { skills, reason } => {
                let mut skills = skills;
                skills.sort();
                assert_eq!(skills, vec!["my-skill", "other-skill"]);
                assert!(reason.contains("--migrate"));
            }
            other => panic!("expected Refused, got {other:?}"),
        }
        // Content untouched.
        assert!(tmp.path().join(".claude/skills/my-skill/SKILL.md").exists());
        assert!(tmp.path().join(".claude/skills/other-skill").is_symlink());
    }

    #[test]
    fn link_agent_refuses_dir_with_only_stray_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills")).unwrap();
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, false) {
            LinkOutcome::Refused { skills, reason } => {
                // A file is not a skill: nothing to migrate, and the hint says so.
                assert!(skills.is_empty());
                assert!(reason.contains("non-skill files"));
                assert!(!reason.contains("--migrate"));
            }
            other => panic!("expected Refused, got {other:?}"),
        }
        // Content untouched.
        assert!(tmp.path().join(".claude/skills/README.txt").exists());
        assert!(!tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_refuses_mixed_skills_and_stray_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        fs::write(tmp.path().join(".claude/skills/my-skill/SKILL.md"), "x").unwrap();
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, false) {
            LinkOutcome::Refused { skills, reason } => {
                assert_eq!(skills, vec!["my-skill"]);
                // The hint mentions both: the stray file must go before --migrate helps.
                assert!(reason.contains("non-skill files"));
                assert!(reason.contains("README.txt"));
                assert!(reason.contains("--migrate"));
            }
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    #[test]
    fn link_agent_migrate_moves_content_and_links() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        // A symlinked skill pointing elsewhere (e.g. into a skills hub) is moved
        // as a link, preserving its target.
        fs::create_dir_all(tmp.path().join("hub/hub-skill")).unwrap();
        std::os::unix::fs::symlink(
            tmp.path().join("hub/hub-skill"),
            tmp.path().join(".claude/skills/hub-skill"),
        )
        .unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, true) {
            LinkOutcome::Migrated { moved, skipped } => {
                let mut moved = moved;
                moved.sort();
                assert_eq!(moved, vec!["hub-skill", "my-skill"]);
                assert!(skipped.is_empty());
            }
            other => panic!("expected Migrated, got {other:?}"),
        }
        assert!(tmp.path().join(".agents/skills/my-skill/SKILL.md").exists());
        let moved_link = tmp.path().join(".agents/skills/hub-skill");
        assert!(moved_link.is_symlink());
        assert_eq!(
            fs::read_link(&moved_link).unwrap(),
            tmp.path().join("hub/hub-skill")
        );
        assert!(tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_migrate_skips_same_name_keeping_canonical() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/pdf");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "agent copy").unwrap();
        // Another skill that does not clash is still migrated.
        fs::create_dir_all(tmp.path().join(".claude/skills/notes")).unwrap();
        fs::write(tmp.path().join(".claude/skills/notes/SKILL.md"), "x").unwrap();
        // Same name already installed in canonical; the canonical copy wins.
        fs::create_dir_all(tmp.path().join(".agents/skills/pdf")).unwrap();
        fs::write(tmp.path().join(".agents/skills/pdf/SKILL.md"), "canonical").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, true) {
            LinkOutcome::Migrated { moved, skipped } => {
                assert_eq!(moved, vec!["notes"]);
                assert_eq!(skipped, vec!["pdf"]);
            }
            other => panic!("expected Migrated, got {other:?}"),
        }
        // Canonical copy untouched; agent dir now links to canonical.
        assert_eq!(
            fs::read_to_string(tmp.path().join(".agents/skills/pdf/SKILL.md")).unwrap(),
            "canonical"
        );
        assert!(tmp.path().join(".claude/skills/notes/SKILL.md").exists());
        assert!(tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_migrate_rejects_stray_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        fs::write(tmp.path().join(".claude/skills/my-skill/SKILL.md"), "x").unwrap();
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        let err = match link_agent(agent, false, &env, true) {
            LinkOutcome::Failed { error } => error,
            other => panic!("expected Failed, got {other:?}"),
        };
        assert!(err.contains("non-skill entries: README.txt"));
        // Nothing moved.
        assert!(tmp.path().join(".claude/skills/my-skill/SKILL.md").exists());
        assert!(!tmp.path().join(".agents/skills").exists());
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
            LinkOutcome::Unlinked
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
            LinkOutcome::NotLinked
        ));
        assert!(tmp.path().join(".claude/skills/my-skill").exists());

        // Universal agents never link.
        let uni = get_agent("amp").unwrap();
        assert!(matches!(
            unlink_agent(uni, false, &env),
            LinkOutcome::NotLinked
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
