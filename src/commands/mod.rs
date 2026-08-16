//! commands: the CLI command layer — arg unpacking + rendering.
//!
//! Business logic lives in the `agent-skill` library (`Manager`), never here.

pub mod add;
pub mod list;
pub mod remove;
pub mod update;

use crate::cli::{DIM, RESET, YELLOW};
use agent_skill::SkillsError;
use agent_skill::error::Result;

/// Render an invalid-agents error to stdout and exit 1 (a CLI-only concern).
pub fn fail_agents(e: SkillsError) -> Result<()> {
    match e {
        SkillsError::InvalidAgents(names) => {
            println!("{YELLOW}Invalid agents: {names}{RESET}");
            println!(
                "{DIM}Valid agents: {}{RESET}",
                agent_skill::core::agents::AGENTS
                    .iter()
                    .map(|a| a.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            std::process::exit(1);
        }
        other => Err(other),
    }
}
