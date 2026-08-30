//! Directory-level agent links: connect each agent's skills dir to the canonical dir.
//!
//! The canonical dir holds the only real copies of installed skills; agents that do
//! not natively read it are integrated with a directory-level symlink
//! ([`link_agent`]): each agent's own skills dir becomes a relative link pointing
//! at the canonical dir, so every install/update/remove is instantly visible to all
//! linked agents.
//!
//! Pre-existing content is never destroyed. A non-empty skills dir is parked
//! whole — a single atomic rename — into the agent's backup slot
//! (`.agents/backup-skills/<agent>/skills`, next to a `manifest.json`). With `migrate`
//! the skill dirs are then adopted out of the slot into the canonical dir (name
//! clashes keep the canonical copy); without it everything stays parked.
//! [`unlink_agent`] disconnects an agent and restores the parked dir with one
//! rename; `--migrate` on an already linked agent pulls parked skills into the
//! canonical dir.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::core::agents::{Agent, Env, agent_skills_dir, canonical_skills_dir};

/// Name of the manifest file inside a backup slot.
const MANIFEST_NAME: &str = "manifest.json";

/// Name of the parked dir inside a backup slot (the agent's skills dir, renamed).
const PARKED_DIR_NAME: &str = "skills";

/// Outcome of linking one agent's skills dir to the canonical dir.
#[derive(Debug)]
pub enum LinkOutcome {
    /// A new directory-level symlink was created. Pre-existing content (if any)
    /// was parked in the agent's backup slot; unlink restores it.
    Linked {
        /// Skill entries parked in the backup slot (reporting only).
        parked_skills: Vec<String>,
        /// Non-skill entries parked in the backup slot (reporting only).
        parked_others: Vec<String>,
        /// The parked dir inside the backup slot (None when nothing was parked).
        backup_dir: Option<PathBuf>,
    },
    /// The agent already uses the canonical dir (linked, or universal).
    AlreadyLinked,
    /// The skills dir was parked whole, then its skill dirs were moved into the
    /// canonical dir (`migrate`); everything else (name-clash copies, non-skill
    /// entries, old-model links) stays parked.
    Migrated {
        /// Names of the skill directories moved into the canonical dir.
        moved: Vec<String>,
        /// Skills whose name already exists in the canonical dir (the canonical
        /// copy wins); the agent-side copy stays parked in the backup slot.
        skipped: Vec<String>,
        /// Non-skill entries parked in the backup slot (reporting only).
        parked_others: Vec<String>,
        /// The parked dir inside the backup slot (None when nothing stays parked).
        backup_dir: Option<PathBuf>,
    },
    /// Linking was refused: the agent dir is a foreign symlink, or a previous
    /// backup is still parked.
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
    /// The agent's skills dir was unlinked from the canonical dir; the parked
    /// dir (if any) was restored with a single rename into its place.
    Unlinked {
        /// Names restored from the backup slot (empty = nothing was parked).
        restored: Vec<String>,
        /// The parked dir the content came from (None when nothing was parked).
        restored_from: Option<PathBuf>,
    },
    /// The agent's skills dir is not a link to the canonical dir (nothing to unlink).
    NotLinked,
}

/// What a backup slot holds (written as `manifest.json` next to the parked dir).
#[derive(Debug, Serialize, Deserialize)]
struct BackupManifest {
    /// Agent whose skills dir was linked.
    agent: String,
    /// Scope of the link (`"project"` or `"global"`).
    scope: String,
    /// Unix timestamp (seconds) of the most recent park.
    created: u64,
    /// Top-level entries parked in the slot at park time.
    backed_up: Vec<String>,
    /// Skills since adopted into the canonical dir (no longer in the slot).
    migrated: Vec<String>,
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
///
/// Content handling: an empty dir is replaced by the link directly; any non-empty
/// dir is parked whole into the agent's backup slot (one atomic rename) before
/// linking. With `migrate`, skill dirs are then adopted into the canonical dir
/// (name clashes keep the canonical copy). Refusal is reserved for a foreign
/// symlink or a previous backup that is still parked.
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
        Err(_) => map_link(
            create_dir_symlink(&canonical, &agent_dir),
            Vec::new(),
            Vec::new(),
            None,
        ),
        Ok(meta) if meta.file_type().is_symlink() => {
            if !points_to(&agent_dir, &canonical) {
                return LinkOutcome::Refused {
                    reason: format!(
                        "{} is a symlink pointing elsewhere; remove it first",
                        agent_dir.display()
                    ),
                };
            }
            // Already linked. With --migrate, pull parked skills out of the
            // backup slot into the canonical dir.
            if migrate {
                migrate_from_backup(agent, global, env, &canonical)
            } else {
                LinkOutcome::AlreadyLinked
            }
        }
        Ok(_) => {
            let entries = match fs::read_dir(&agent_dir) {
                Err(e) => {
                    return LinkOutcome::Failed {
                        error: e.to_string(),
                    };
                }
                Ok(rd) => rd.flatten().collect::<Vec<_>>(),
            };
            if entries.is_empty() {
                // Empty dir: safe to replace with the link.
                let _ = fs::remove_dir(&agent_dir);
                return map_link(
                    create_dir_symlink(&canonical, &agent_dir),
                    Vec::new(),
                    Vec::new(),
                    None,
                );
            }

            // Classification is for reporting and migrate decisions only — the
            // whole dir is parked either way.
            let (skills, others) = classify(&entries, &canonical);
            let names: Vec<String> = entries.iter().map(entry_name).collect();
            let slot = backup_slot(agent, global, env);
            let parked = slot.join(PARKED_DIR_NAME);
            if !parked_entries(&parked).is_empty() {
                return LinkOutcome::Refused {
                    reason: format!(
                        "a previous backup is still parked at {}; move it away or remove it before linking {}",
                        parked.display(),
                        agent_dir.display()
                    ),
                };
            }

            if let Some(failed) = park_dir(agent, global, env, &agent_dir, &names, &[]) {
                return failed;
            }
            let (moved, skipped) = if migrate {
                match adopt_skills(&canonical, &parked, &skills) {
                    Ok(pair) => pair,
                    Err(error) => return LinkOutcome::Failed { error },
                }
            } else {
                (Vec::new(), Vec::new())
            };

            let res = create_dir_symlink(&canonical, &agent_dir);
            if migrate {
                let remaining: Vec<String> = names
                    .iter()
                    .filter(|n| !moved.contains(n))
                    .cloned()
                    .collect();
                let backup_dir = if remaining.is_empty() {
                    cleanup_slot(&slot);
                    None
                } else {
                    rewrite_manifest(&slot, agent, global, &moved, &remaining);
                    Some(parked)
                };
                match res {
                    Ok(()) => {
                        return LinkOutcome::Migrated {
                            moved,
                            skipped,
                            parked_others: others,
                            backup_dir,
                        };
                    }
                    Err(error) => return LinkOutcome::Failed { error },
                }
            }
            map_link(res, skills, others, Some(parked))
        }
    }
}

/// Unlink an agent's skills dir from the canonical dir, restoring the parked
/// dir (if any) with a single rename into its place.
///
/// Returns a [`LinkOutcome`]: [`LinkOutcome::Unlinked`] on success,
/// [`LinkOutcome::NotLinked`] when there is nothing to do. A real skills dir is
/// replaced only when a backup is pending and it is empty (or the restore fails
/// with a clear error); foreign symlinks are left alone.
pub fn unlink_agent(agent: &Agent, global: bool, env: &Env) -> LinkOutcome {
    // Universal agents use the canonical dir natively — nothing to unlink.
    if agent.is_universal() {
        return LinkOutcome::NotLinked;
    }

    let Some(agent_dir) = agent_skills_dir(agent, global, env) else {
        return LinkOutcome::NotLinked;
    };
    let canonical = canonical_skills_dir(global, env);
    let slot = backup_slot(agent, global, env);
    let pending = !parked_entries(&slot.join(PARKED_DIR_NAME)).is_empty();

    match fs::symlink_metadata(&agent_dir) {
        // Dir gone: restore only when a backup is pending.
        Err(_) => {
            if pending {
                restore_backup(&slot, &agent_dir)
            } else {
                LinkOutcome::NotLinked
            }
        }
        Ok(meta) if meta.file_type().is_symlink() => {
            if !points_to(&agent_dir, &canonical) {
                // A foreign symlink: leave it alone.
                return LinkOutcome::NotLinked;
            }
            if let Err(e) = fs::remove_file(&agent_dir) {
                return LinkOutcome::Failed {
                    error: e.to_string(),
                };
            }
            restore_backup(&slot, &agent_dir)
        }
        Ok(_) => {
            if pending {
                restore_backup(&slot, &agent_dir)
            } else {
                LinkOutcome::NotLinked
            }
        }
    }
}

/// Classify the private content of an unlinked agent's skills dir:
/// `(skills, other entries)`, using the same rules as migrate.
pub fn private_content(agent: &Agent, global: bool, env: &Env) -> (Vec<String>, Vec<String>) {
    let Some(dir) = agent_skills_dir(agent, global, env) else {
        return (Vec::new(), Vec::new());
    };
    // A symlinked skills dir is not private content (whatever it points at).
    if fs::symlink_metadata(&dir)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return (Vec::new(), Vec::new());
    }
    let Ok(rd) = fs::read_dir(&dir) else {
        return (Vec::new(), Vec::new());
    };
    let entries: Vec<_> = rd.flatten().collect();
    if entries.is_empty() {
        return (Vec::new(), Vec::new());
    }
    classify(&entries, &canonical_skills_dir(global, env))
}

/// The agent's parked dir with content, if any (for `--status`).
pub fn pending_backup(agent: &Agent, global: bool, env: &Env) -> Option<(PathBuf, Vec<String>)> {
    let parked = backup_slot(agent, global, env).join(PARKED_DIR_NAME);
    let entries = parked_entries(&parked);
    if entries.is_empty() {
        return None;
    }
    let items = entries.iter().map(entry_name).collect();
    Some((parked, items))
}

/// Backup root for parked agent dirs: `(global ? home : cwd)/.agents/backup-skills`.
fn backup_root(global: bool, env: &Env) -> PathBuf {
    canonical_skills_dir(global, env)
        .parent()
        .unwrap_or(Path::new(".agents"))
        .join("backup-skills")
}

/// One agent's backup slot: `<backup root>/<agent name>`.
fn backup_slot(agent: &Agent, global: bool, env: &Env) -> PathBuf {
    backup_root(global, env).join(&agent.name)
}

/// Entries inside the parked dir (a missing dir is empty).
fn parked_entries(parked: &Path) -> Vec<fs::DirEntry> {
    fs::read_dir(parked)
        .map(|rd| rd.flatten().collect())
        .unwrap_or_default()
}

/// Rename the whole agent dir into the agent's backup slot and write the
/// manifest. `None` means success; `Some` is a [`LinkOutcome::Failed`].
fn park_dir(
    agent: &Agent,
    global: bool,
    env: &Env,
    agent_dir: &Path,
    names: &[String],
    migrated: &[String],
) -> Option<LinkOutcome> {
    let slot = backup_slot(agent, global, env);
    if let Err(e) = fs::create_dir_all(&slot) {
        return Some(LinkOutcome::Failed {
            error: format!("create {}: {e}", slot.display()),
        });
    }
    ensure_backup_gitignore(global, env);
    let parked = slot.join(PARKED_DIR_NAME);
    // A degenerate leftover (empty parked dir) does not block parking.
    let _ = fs::remove_dir(&parked);
    if let Err(e) = fs::rename(agent_dir, &parked) {
        return Some(LinkOutcome::Failed {
            error: format!("park {}: {e}", agent_dir.display()),
        });
    }
    write_manifest(&slot, agent, global, migrated, names);
    None
}

/// Move skill dirs out of the parked dir into the canonical dir (name clashes
/// keep the canonical copy — those stay parked). Returns `(moved, skipped)`.
fn adopt_skills(
    canonical: &Path,
    parked: &Path,
    skills: &[String],
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut moved = Vec::new();
    let mut skipped = Vec::new();
    if skills.is_empty() {
        return Ok((moved, skipped));
    }
    fs::create_dir_all(canonical).map_err(|e| format!("create {}: {e}", canonical.display()))?;
    for name in skills {
        let from = parked.join(name);
        if canonical.join(name).exists() {
            skipped.push(name.clone());
        } else if let Err(e) = fs::rename(&from, canonical.join(name)) {
            return Err(format!("move {}: {e}", from.display()));
        } else {
            moved.push(name.clone());
        }
    }
    Ok((moved, skipped))
}

/// The agent dir is already linked to the canonical dir; with `--migrate`, pull
/// parked skill dirs out of the backup slot into the canonical dir. Name-clash
/// copies, non-skill entries and old-model links stay parked.
fn migrate_from_backup(agent: &Agent, global: bool, env: &Env, canonical: &Path) -> LinkOutcome {
    let slot = backup_slot(agent, global, env);
    let parked = slot.join(PARKED_DIR_NAME);
    let entries = parked_entries(&parked);
    if entries.is_empty() {
        cleanup_slot(&slot);
        return LinkOutcome::AlreadyLinked;
    }
    let names: Vec<String> = entries.iter().map(entry_name).collect();
    let (skills, others) = classify(&entries, canonical);
    let (moved, skipped) = match adopt_skills(canonical, &parked, &skills) {
        Ok(pair) => pair,
        Err(error) => return LinkOutcome::Failed { error },
    };
    let remaining: Vec<String> = names
        .iter()
        .filter(|n| !moved.contains(n))
        .cloned()
        .collect();
    if remaining.is_empty() {
        cleanup_slot(&slot);
        return LinkOutcome::Migrated {
            moved,
            skipped,
            parked_others: others,
            backup_dir: None,
        };
    }
    rewrite_manifest(&slot, agent, global, &moved, &remaining);
    LinkOutcome::Migrated {
        moved,
        skipped,
        parked_others: others,
        backup_dir: Some(parked),
    }
}

/// Restore the parked dir into `agent_dir` with a single atomic rename, then
/// drop the slot. Nothing parked → a fresh empty dir.
fn restore_backup(slot: &Path, agent_dir: &Path) -> LinkOutcome {
    let parked = slot.join(PARKED_DIR_NAME);
    let restored: Vec<String> = parked_entries(&parked).iter().map(entry_name).collect();
    if restored.is_empty() {
        cleanup_slot(slot);
        // Recreate an empty dir so the agent does not see a missing skills dir.
        if let Err(e) = fs::create_dir_all(agent_dir) {
            return LinkOutcome::Failed {
                error: e.to_string(),
            };
        }
        return LinkOutcome::Unlinked {
            restored: Vec::new(),
            restored_from: None,
        };
    }
    // The target must be gone (or an empty leftover dir).
    match fs::symlink_metadata(agent_dir) {
        Err(_) => {}
        Ok(m) if m.is_dir() => {
            let is_empty = fs::read_dir(agent_dir)
                .map(|mut rd| rd.next().is_none())
                .unwrap_or(false);
            if !is_empty {
                return LinkOutcome::Failed {
                    error: format!(
                        "restore blocked: {} exists and is not empty; move the backup at {} manually",
                        agent_dir.display(),
                        parked.display()
                    ),
                };
            }
            if let Err(e) = fs::remove_dir(agent_dir) {
                return LinkOutcome::Failed {
                    error: e.to_string(),
                };
            }
        }
        Ok(_) => {
            return LinkOutcome::Failed {
                error: format!("restore blocked: {} exists", agent_dir.display()),
            };
        }
    }
    if let Err(e) = fs::rename(&parked, agent_dir) {
        return LinkOutcome::Failed {
            error: format!("restore {}: {e}", parked.display()),
        };
    }
    cleanup_slot(slot);
    LinkOutcome::Unlinked {
        restored,
        restored_from: Some(parked),
    }
}

/// Remove the manifest, the parked dir, the slot itself, and the backup root
/// when this was the last slot (each step only succeeds when empty).
fn cleanup_slot(slot: &Path) {
    let _ = fs::remove_file(slot.join(MANIFEST_NAME));
    let _ = fs::remove_dir(slot.join(PARKED_DIR_NAME));
    let _ = fs::remove_dir(slot);
    if let Some(root) = slot.parent() {
        let _ = fs::remove_dir(root);
    }
}

/// Write the slot manifest recording what was parked vs. adopted.
fn write_manifest(
    slot: &Path,
    agent: &Agent,
    global: bool,
    migrated: &[String],
    backed_up: &[String],
) {
    let manifest = BackupManifest {
        agent: agent.name.to_string(),
        scope: if global { "global" } else { "project" }.to_string(),
        created: now_secs(),
        backed_up: backed_up.to_vec(),
        migrated: migrated.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&manifest) {
        let _ = fs::write(slot.join(MANIFEST_NAME), json);
    }
}

/// Rewrite the slot manifest after some parked skills were adopted out.
fn rewrite_manifest(
    slot: &Path,
    agent: &Agent,
    global: bool,
    moved_out: &[String],
    remaining: &[String],
) {
    let previous = fs::read_to_string(slot.join(MANIFEST_NAME))
        .ok()
        .and_then(|s| serde_json::from_str::<BackupManifest>(&s).ok());
    let mut migrated = previous.map(|m| m.migrated).unwrap_or_default();
    migrated.extend(moved_out.iter().cloned());
    write_manifest(slot, agent, global, &migrated, remaining);
}

/// Keep the project-scope backup root out of version control (the `.agents/`
/// dir itself is normally committed).
fn ensure_backup_gitignore(global: bool, env: &Env) {
    if global {
        return;
    }
    let root = backup_root(global, env);
    let gitignore = root.join(".gitignore");
    if !gitignore.exists()
        && let Err(e) =
            fs::create_dir_all(&root).and_then(|_| fs::write(&gitignore, "*\n!.gitignore\n"))
    {
        debug_assert!(false, "write backup .gitignore: {e}");
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Map a symlink-creation result to a `Linked`/`Failed` outcome.
fn map_link(
    res: Result<(), String>,
    parked_skills: Vec<String>,
    parked_others: Vec<String>,
    backup_dir: Option<PathBuf>,
) -> LinkOutcome {
    match res {
        Ok(()) => LinkOutcome::Linked {
            parked_skills,
            parked_others,
            backup_dir,
        },
        Err(error) => LinkOutcome::Failed { error },
    }
}

fn entry_name(entry: &fs::DirEntry) -> String {
    entry.file_name().to_string_lossy().into_owned()
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

/// Split dir entries into `(skills, others)`: skills are real subdirs and
/// symlinks whose target is a directory (e.g. links into a skills hub); others
/// are files and symlinks to non-directories. Old-model per-skill links into
/// the canonical dir are excluded from both (their content already lives there).
fn classify(entries: &[fs::DirEntry], canonical: &Path) -> (Vec<String>, Vec<String>) {
    let mut skills = Vec::new();
    let mut others = Vec::new();
    for entry in entries {
        if is_legacy_link(entry, canonical) {
            continue;
        }
        let name = entry_name(entry);
        if fs::metadata(entry.path())
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            skills.push(name);
        } else {
            others.push(name);
        }
    }
    (skills, others)
}

/// Create `link` as a symlink to `canonical`, using a relative target when possible.
fn create_dir_symlink(canonical: &Path, link: &Path) -> Result<(), String> {
    if let Some(parent) = link.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return Err(format!("create {}: {e}", parent.display()));
    }
    let target = relative_target(canonical, link);
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(&target, link);
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_dir(&target, link);
    result.map_err(|e| {
        format!(
            "symlink {} -> {}: {e} (on Windows, enable Developer Mode to allow symlinks)",
            link.display(),
            target.display()
        )
    })
}

/// Relative path from `link`'s parent to `canonical` (absolute fallback).
fn relative_target(canonical: &Path, link: &Path) -> PathBuf {
    let base = link.parent().unwrap_or(Path::new("."));
    pathdiff::diff_paths(canonical, base).unwrap_or_else(|| canonical.to_path_buf())
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

    /// Sorted copy of a names vec (read_dir order is arbitrary).
    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    #[test]
    fn link_agent_creates_relative_symlink_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".windsurf")).unwrap();
        let agent = get_agent("windsurf").unwrap();

        let outcome = link_agent(agent, false, &env, false);
        assert!(
            matches!(
                outcome,
                LinkOutcome::Linked {
                    backup_dir: None,
                    ..
                }
            ),
            "got {outcome:?}"
        );
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
        assert!(matches!(outcome, LinkOutcome::Linked { .. }));
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
            LinkOutcome::Linked { .. }
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
            LinkOutcome::Refused { .. }
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
            LinkOutcome::Linked { .. }
        ));
        assert!(tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_parks_existing_content_and_links() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        // A symlinked skill pointing elsewhere (e.g. into a skills hub) is parked as is.
        fs::create_dir_all(tmp.path().join("hub/other-skill")).unwrap();
        std::os::unix::fs::symlink(
            tmp.path().join("hub/other-skill"),
            tmp.path().join(".claude/skills/other-skill"),
        )
        .unwrap();
        // A stray file is parked too.
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, false) {
            LinkOutcome::Linked {
                parked_skills,
                parked_others,
                backup_dir,
            } => {
                assert_eq!(sorted(parked_skills), vec!["my-skill", "other-skill"]);
                assert_eq!(parked_others, vec!["README.txt"]);
                assert_eq!(
                    backup_dir,
                    Some(tmp.path().join(".agents/backup-skills/claude-code/skills"))
                );
            }
            other => panic!("expected Linked, got {other:?}"),
        }
        // The whole dir was parked as is; the agent dir is linked.
        assert!(tmp.path().join(".claude/skills").is_symlink());
        let parked = tmp.path().join(".agents/backup-skills/claude-code/skills");
        assert!(parked.join("my-skill/SKILL.md").exists());
        assert!(parked.join("other-skill").is_symlink());
        assert!(parked.join("README.txt").exists());
        assert!(
            tmp.path()
                .join(".agents/backup-skills/claude-code/manifest.json")
                .exists()
        );
        // Skills are NOT in the canonical dir (plain link only parks).
        assert!(!tmp.path().join(".agents/skills").exists());
    }

    #[test]
    fn link_agent_parks_dir_with_only_stray_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills")).unwrap();
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, false) {
            LinkOutcome::Linked {
                parked_skills,
                parked_others,
                ..
            } => {
                assert!(parked_skills.is_empty());
                assert_eq!(parked_others, vec!["README.txt"]);
            }
            other => panic!("expected Linked, got {other:?}"),
        }
        assert!(tmp.path().join(".claude/skills").is_symlink());
        assert!(
            tmp.path()
                .join(".agents/backup-skills/claude-code/skills/README.txt")
                .exists()
        );
    }

    #[test]
    fn link_agent_refuses_stale_backup_slot() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills/old")).unwrap();
        // A previous park left content behind.
        let parked = tmp.path().join(".agents/backup-skills/claude-code/skills");
        fs::create_dir_all(&parked).unwrap();
        fs::write(parked.join("parked.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, false) {
            LinkOutcome::Refused { reason } => {
                assert!(reason.contains("backup"), "reason: {reason}");
            }
            other => panic!("expected Refused, got {other:?}"),
        }
        // Nothing was touched.
        assert!(tmp.path().join(".claude/skills/old").exists());
        assert!(!tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_degenerate_empty_parked_dir_does_not_block() {
        // A leftover empty parked dir is not a backup; parking replaces it.
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        fs::create_dir_all(tmp.path().join(".agents/backup-skills/claude-code/skills")).unwrap();
        let agent = get_agent("claude-code").unwrap();

        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));
        assert!(tmp.path().join(".claude/skills").is_symlink());
        assert!(
            tmp.path()
                .join(".agents/backup-skills/claude-code/skills/my-skill/SKILL.md")
                .exists()
        );
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
            LinkOutcome::Migrated {
                moved,
                skipped,
                parked_others,
                backup_dir,
            } => {
                assert_eq!(sorted(moved), vec!["hub-skill", "my-skill"]);
                assert!(skipped.is_empty());
                assert!(parked_others.is_empty());
                assert!(backup_dir.is_none(), "nothing parked, no slot needed");
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
        assert!(
            !tmp.path()
                .join(".agents/backup-skills/claude-code")
                .exists()
        );
    }

    #[test]
    fn link_agent_migrate_skips_same_name_parking_agent_copy() {
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
            LinkOutcome::Migrated {
                moved,
                skipped,
                parked_others,
                backup_dir,
            } => {
                assert_eq!(moved, vec!["notes"]);
                assert_eq!(skipped, vec!["pdf"]);
                assert!(parked_others.is_empty());
                assert_eq!(
                    backup_dir,
                    Some(tmp.path().join(".agents/backup-skills/claude-code/skills"))
                );
            }
            other => panic!("expected Migrated, got {other:?}"),
        }
        // Canonical copy untouched; the agent-side copy is parked for restore.
        assert_eq!(
            fs::read_to_string(tmp.path().join(".agents/skills/pdf/SKILL.md")).unwrap(),
            "canonical"
        );
        assert_eq!(
            fs::read_to_string(
                tmp.path()
                    .join(".agents/backup-skills/claude-code/skills/pdf/SKILL.md")
            )
            .unwrap(),
            "agent copy"
        );
        assert!(tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_migrate_parks_stray_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        fs::write(tmp.path().join(".claude/skills/my-skill/SKILL.md"), "x").unwrap();
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, true) {
            LinkOutcome::Migrated {
                moved,
                skipped: _,
                parked_others,
                backup_dir,
            } => {
                assert_eq!(moved, vec!["my-skill"]);
                assert_eq!(parked_others, vec!["README.txt"]);
                assert_eq!(
                    backup_dir,
                    Some(tmp.path().join(".agents/backup-skills/claude-code/skills"))
                );
            }
            other => panic!("expected Migrated, got {other:?}"),
        }
        assert!(tmp.path().join(".agents/skills/my-skill/SKILL.md").exists());
        assert!(
            tmp.path()
                .join(".agents/backup-skills/claude-code/skills/README.txt")
                .exists()
        );
        assert!(tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_migrate_pulls_parked_skills_from_backup() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        // Plain link first: the skill is parked, not migrated.
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));
        assert!(!tmp.path().join(".agents/skills").exists());

        // Rerunning with --migrate on the already linked agent moves the parked
        // skill into the canonical dir; the stray file stays parked.
        match link_agent(agent, false, &env, true) {
            LinkOutcome::Migrated {
                moved,
                skipped,
                parked_others,
                backup_dir,
            } => {
                assert_eq!(moved, vec!["my-skill"]);
                assert!(skipped.is_empty());
                assert_eq!(parked_others, vec!["README.txt"]);
                assert!(backup_dir.is_some());
            }
            other => panic!("expected Migrated, got {other:?}"),
        }
        assert!(tmp.path().join(".agents/skills/my-skill/SKILL.md").exists());
        assert!(
            tmp.path()
                .join(".agents/backup-skills/claude-code/skills/README.txt")
                .exists()
        );
        assert!(tmp.path().join(".claude/skills").is_symlink());
    }

    #[test]
    fn link_agent_migrate_already_linked_without_backup() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let agent = get_agent("claude-code").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));
        // --migrate on a linked agent with an empty slot stays AlreadyLinked.
        assert!(matches!(
            link_agent(agent, false, &env, true),
            LinkOutcome::AlreadyLinked
        ));
    }

    #[test]
    fn link_agent_parks_legacy_per_skill_links() {
        // Old-model agent dirs (per-skill links into canonical) are parked whole
        // and restored as is — nothing is dropped.
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

        match link_agent(agent, false, &env, false) {
            LinkOutcome::Linked { backup_dir, .. } => {
                assert!(
                    backup_dir.is_some(),
                    "legacy links are parked, not taken over"
                );
            }
            other => panic!("expected Linked, got {other:?}"),
        }
        let link = tmp.path().join(".windsurf/skills");
        assert!(link.is_symlink());
        assert!(link.join("pdf/SKILL.md").exists());
        assert!(
            tmp.path()
                .join(".agents/backup-skills/windsurf/skills/pdf")
                .is_symlink()
        );
    }

    #[test]
    fn link_agent_migrate_keeps_legacy_links_parked() {
        // Legacy per-skill links already point into the canonical dir: --migrate
        // leaves them parked instead of moving them into canonical.
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let src = tmp.path().join("src-skill");
        let skill = write_and_parse_skill(&src, "pdf");
        install_skill(&skill, false, &env);
        // The agent dir holds a legacy link plus a real skill of its own.
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        std::os::unix::fs::symlink(
            tmp.path().join(".agents/skills/pdf"),
            tmp.path().join(".claude/skills/pdf"),
        )
        .unwrap();
        let agent = get_agent("claude-code").unwrap();

        match link_agent(agent, false, &env, true) {
            LinkOutcome::Migrated {
                moved,
                skipped,
                backup_dir,
                ..
            } => {
                assert_eq!(moved, vec!["my-skill"]);
                assert!(skipped.is_empty());
                assert!(backup_dir.is_some(), "legacy link stays parked");
            }
            other => panic!("expected Migrated, got {other:?}"),
        }
        assert!(tmp.path().join(".agents/skills/my-skill/SKILL.md").exists());
        let parked_link = tmp
            .path()
            .join(".agents/backup-skills/claude-code/skills/pdf");
        assert!(parked_link.is_symlink());
    }

    #[test]
    fn unlink_agent_restores_parked_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        fs::write(tmp.path().join(".claude/skills/notes.txt"), "n").unwrap();
        let agent = get_agent("claude-code").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));

        match unlink_agent(agent, false, &env) {
            LinkOutcome::Unlinked {
                restored,
                restored_from,
            } => {
                assert_eq!(sorted(restored), vec!["my-skill", "notes.txt"]);
                assert_eq!(
                    restored_from,
                    Some(tmp.path().join(".agents/backup-skills/claude-code/skills"))
                );
            }
            other => panic!("expected Unlinked, got {other:?}"),
        }
        // Content is back in a real dir; the slot is gone.
        let dir = tmp.path().join(".claude/skills");
        assert!(dir.is_dir());
        assert!(!dir.is_symlink());
        assert!(dir.join("my-skill/SKILL.md").exists());
        assert!(dir.join("notes.txt").exists());
        assert!(
            !tmp.path()
                .join(".agents/backup-skills/claude-code")
                .exists()
        );
    }

    #[test]
    fn unlink_agent_removes_link_and_recreates_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".windsurf")).unwrap();
        let agent = get_agent("windsurf").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));

        match unlink_agent(agent, false, &env) {
            LinkOutcome::Unlinked {
                restored,
                restored_from,
            } => {
                assert!(restored.is_empty());
                assert!(restored_from.is_none());
            }
            other => panic!("expected Unlinked, got {other:?}"),
        }
        let dir = tmp.path().join(".windsurf/skills");
        assert!(dir.is_dir());
        assert!(!dir.is_symlink());
        assert!(fs::read_dir(&dir).unwrap().count() == 0);
    }

    #[test]
    fn unlink_agent_recovers_missing_dir_from_backup() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));
        // Simulate the link being removed without an unlink: the dir is gone
        // but the backup is pending.
        fs::remove_file(tmp.path().join(".claude/skills")).unwrap();

        match unlink_agent(agent, false, &env) {
            LinkOutcome::Unlinked { restored, .. } => {
                assert_eq!(restored, vec!["my-skill"]);
            }
            other => panic!("expected Unlinked, got {other:?}"),
        }
        assert!(tmp.path().join(".claude/skills/my-skill/SKILL.md").exists());
        assert!(
            !tmp.path()
                .join(".agents/backup-skills/claude-code")
                .exists()
        );
    }

    #[test]
    fn unlink_agent_replaces_empty_real_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));
        // The user removed the link and left an empty real dir behind.
        fs::remove_file(tmp.path().join(".claude/skills")).unwrap();
        fs::create_dir(tmp.path().join(".claude/skills")).unwrap();

        match unlink_agent(agent, false, &env) {
            LinkOutcome::Unlinked { restored, .. } => {
                assert_eq!(restored, vec!["my-skill"]);
            }
            other => panic!("expected Unlinked, got {other:?}"),
        }
        assert!(tmp.path().join(".claude/skills/my-skill/SKILL.md").exists());
    }

    #[test]
    fn unlink_agent_blocked_by_real_dir_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));
        // The user removed the link and put their own content in the way.
        fs::remove_file(tmp.path().join(".claude/skills")).unwrap();
        let dir = tmp.path().join(".claude/skills");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("mine.txt"), "x").unwrap();

        match unlink_agent(agent, false, &env) {
            LinkOutcome::Failed { error } => {
                assert!(error.contains("restore blocked"), "error: {error}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        // Nothing was touched: user content and backup both intact.
        assert!(dir.join("mine.txt").exists());
        assert!(
            tmp.path()
                .join(".agents/backup-skills/claude-code/skills/my-skill")
                .exists()
        );
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
    fn unlink_agent_keeps_migrated_skills_in_canonical() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        let existing = tmp.path().join(".claude/skills/my-skill");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("SKILL.md"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();
        assert!(matches!(
            link_agent(agent, false, &env, true),
            LinkOutcome::Migrated { .. }
        ));

        match unlink_agent(agent, false, &env) {
            LinkOutcome::Unlinked { restored, .. } => {
                assert!(restored.is_empty(), "migrated skills are not restored");
            }
            other => panic!("expected Unlinked, got {other:?}"),
        }
        // The migrated skill stays in the canonical dir (managed by `remove`).
        assert!(tmp.path().join(".agents/skills/my-skill/SKILL.md").exists());
        let dir = tmp.path().join(".claude/skills");
        assert!(dir.is_dir());
        assert!(!dir.is_symlink());
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
            LinkOutcome::Linked { .. }
        ));
        assert!(is_agent_linked(agent, false, &env));
        // Universal agents are always "linked" (canonical is their dir).
        assert!(is_agent_linked(get_agent("amp").unwrap(), false, &env));
    }

    #[test]
    fn private_content_classifies_skills_and_others() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        fs::write(tmp.path().join(".claude/skills/my-skill/SKILL.md"), "x").unwrap();
        fs::write(tmp.path().join(".claude/skills/README.txt"), "x").unwrap();
        let agent = get_agent("claude-code").unwrap();

        let (skills, others) = private_content(agent, false, &env);
        assert_eq!(skills, vec!["my-skill"]);
        assert_eq!(others, vec!["README.txt"]);

        // A linked (or foreign-symlink) skills dir is not private content.
        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));
        assert_eq!(
            private_content(agent, false, &env),
            (Vec::new(), Vec::new())
        );
    }

    #[test]
    fn pending_backup_reports_parked_slot() {
        let tmp = tempfile::TempDir::new().unwrap();
        let env = env_at(&tmp);
        fs::create_dir_all(tmp.path().join(".claude/skills/my-skill")).unwrap();
        let agent = get_agent("claude-code").unwrap();
        assert!(pending_backup(agent, false, &env).is_none());

        assert!(matches!(
            link_agent(agent, false, &env, false),
            LinkOutcome::Linked { .. }
        ));
        let (parked, items) = pending_backup(agent, false, &env).expect("pending backup");
        assert_eq!(
            parked,
            tmp.path().join(".agents/backup-skills/claude-code/skills")
        );
        assert_eq!(items, vec!["my-skill"]);

        // After unlink the slot is gone.
        assert!(matches!(
            unlink_agent(agent, false, &env),
            LinkOutcome::Unlinked { .. }
        ));
        assert!(pending_backup(agent, false, &env).is_none());
    }
}
