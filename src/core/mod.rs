//! core: CLI-agnostic domain logic (source parsing, agent dirs, SKILL.md discovery, install, lock).
//!
//! Everything is pure functions or dependency-injectable, for easy unit testing and reuse.

pub mod agents;
pub mod discover;
pub mod fetch;
pub mod install;
pub mod lock;
pub mod source;

#[cfg(test)]
pub mod test_utils;
