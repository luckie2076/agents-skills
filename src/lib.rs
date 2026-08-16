//! `agent-skill` as a library: a high-level [`Manager`] facade plus low-level `core` primitives.
//!
//! The library is pure data — it never prints to stdout/stderr and never calls
//! `process::exit`. The CLI binary (see `src/main.rs` + `src/commands`) is responsible
//! for rendering outcomes and deciding exit codes.

#![warn(missing_docs)]

pub mod core;
pub mod error;
pub mod manager;

// High-level facade.
pub use manager::{
    AddOutcome, AddRequest, InstallFailure, InstallSuccess, ListRequest, ListedSkill, Manager,
    ManagerBuilder, RemoveOutcome, RemoveRequest, UpdateOutcome, UpdateRequest,
};

// Errors.
pub use error::{Error, Result, SkillsError};

// Low-level primitives.
pub use core::agents::{
    Agent, Env, GlobalDir, agent_display, detect_installed_agents, ensure_universal_agents,
    get_agent,
};
pub use core::discover::{Skill, discover_skills, filter_skills, parse_skill_md};
pub use core::install::{
    InstallMode, InstallResult, InstalledSkill, find_skill, install_skill_for_agent,
    list_installed_skills, matches_skill, resolve_to_remove, sanitize_name, scan_installed,
};
pub use core::lock::{
    LocalLockFile, LockEntry, compute_folder_hash, find_lock_entry, global_lock_path,
    local_lock_path, lock_fields, read_local_lock, write_local_lock,
};
pub use core::source::{Source, SourceType, owner_repo, parse_source};
