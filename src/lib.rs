//! `agents-skills` as a library: a high-level [`Manager`] facade over the low-level [`core`] module.
//!
//! The library is pure data — it never prints to stdout/stderr and never calls
//! `process::exit`. The CLI binary (see `src/main.rs` + `src/commands`) is responsible
//! for rendering outcomes and deciding exit codes.
//!
//! # Quick tour
//!
//! ```
//! use agents_skills::{ListRequest, Manager};
//!
//! // Point the manager at a scratch environment (hermetic, no real home access).
//! let manager = Manager::builder()
//!     .home("/tmp/home")
//!     .config("/tmp/config")
//!     .cwd("/tmp/project")
//!     .build();
//!
//! // Install every skill from a source (see [`Manager::add`] for a local example).
//! // List what's installed.
//! let skills = manager.list(&ListRequest::default())?;
//!
//! # Ok::<(), agents_skills::Error>(())
//! ```
//!
//! # Layering
//!
//! The crate root exposes only the high-level [`Manager`] facade, its request/outcome types,
//! and the unified [`error`] types. Lower-level primitives (source parsing, agent directories,
//! SKILL.md discovery, install, agent links, lock) live under [`core`] and are accessed as
//! `agents_skills::core::...`.

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

pub mod core;
pub mod error;
pub mod manager;

// High-level facade.
pub use manager::{
    AddOutcome, AddRequest, AgentLinkResult, InstallFailure, InstallSuccess, LinkManagerOutcome,
    LinkRequest, LinkStatus, ListRequest, ListedSkill, Manager, ManagerBuilder, RemoveOutcome,
    RemoveRequest, Scope, UpdateOutcome, UpdateRequest,
};

// Link/unlink outcome enum (surfaced by the facade's link result types).
pub use core::link::LinkOutcome;

// Errors.
pub use error::{Error, Result, SkillsError};
