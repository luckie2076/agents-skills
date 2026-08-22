//! commands: the CLI command layer — arg unpacking + rendering.
//!
//! Business logic lives in the `agents-skills` library (`Manager`), never here.
//! Shared rendering helpers (path shortening, link result lines) live in this module.

pub mod add;
pub mod link;
pub mod list;
pub mod remove;
pub mod update;

use std::path::Path;

use crate::cli::{DIM, GREEN, RED, RESET, YELLOW};
use agents_skills::core::agents::Env;
use agents_skills::error::Result;
use agents_skills::{AgentLinkResult, LinkOutcome, SkillsError};

/// Render an invalid-agents error to stdout and exit 1 (a CLI-only concern).
pub fn fail_agents(e: SkillsError) -> Result<()> {
    match e {
        SkillsError::InvalidAgents(names) => {
            println!("{YELLOW}Invalid agents: {names}{RESET}");
            println!(
                "{DIM}Valid agents: {}{RESET}",
                agents_skills::core::agents::AGENTS
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

/// Render one agent link result line (used by `link`).
pub fn render_link_result(r: &AgentLinkResult) {
    match &r.outcome {
        LinkOutcome::Linked => println!("{GREEN}✓{RESET} {DIM}{}{RESET} linked", r.display),
        LinkOutcome::AlreadyLinked => {
            println!("{DIM}• {} already linked (canonical dir){RESET}", r.display)
        }
        LinkOutcome::Migrated { moved } => println!(
            "{GREEN}✓{RESET} {} linked {DIM}(migrated {} skill{}){RESET}",
            r.display,
            moved.join(", "),
            if moved.len() != 1 { "s" } else { "" }
        ),
        LinkOutcome::Refused { reason } => println!("{YELLOW}!{RESET} {} {reason}", r.display),
        LinkOutcome::Skipped => println!("{DIM}– {} skipped (not installed){RESET}", r.display),
        LinkOutcome::Failed { error } => println!("{RED}✗{RESET} {}: {error}", r.display),
    }
}

/// Shorten a path for display: `~` for home, `.` for cwd prefixes.
pub fn shorten_path(path: &Path, env: &Env) -> String {
    let full = path.to_string_lossy();
    let home_s = env.home.to_string_lossy();
    let cwd_s = env.cwd.to_string_lossy();
    if full == home_s {
        return "~".to_string();
    }
    if let Some(rest) = full.strip_prefix(&*home_s)
        && (rest.starts_with('/') || rest.starts_with('\\'))
    {
        return format!("~{rest}");
    }
    if full == cwd_s {
        return ".".to_string();
    }
    if let Some(rest) = full.strip_prefix(&*cwd_s)
        && (rest.starts_with('/') || rest.starts_with('\\'))
    {
        return format!(".{rest}");
    }
    full.to_string()
}
