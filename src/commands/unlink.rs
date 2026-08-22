//! unlink: disconnect agents' skills dirs from the canonical dir.
//!
//! Renders the [`Manager::unlink`] outcome; no business logic lives here.

use crate::cli::{DIM, GREEN, RED, RESET};
use crate::commands::fail_agents;
use agents_skills::error::Result;
use agents_skills::{Manager, UnlinkManagerOutcome, UnlinkOutcome, UnlinkRequest};

pub fn run(manager: &Manager, args: crate::cli::UnlinkArgs) -> Result<()> {
    let req = UnlinkRequest {
        agents: args.agents,
        global: args.global,
    };
    let outcome = match manager.unlink(&req) {
        Ok(o) => o,
        Err(e) => return fail_agents(e),
    };
    render(&outcome);
    Ok(())
}

fn render(outcome: &UnlinkManagerOutcome) {
    let scope = if outcome.global { "global" } else { "project" };
    println!("{DIM}Unlinking agents from the {scope} canonical skills dir{RESET}");
    println!();
    for r in &outcome.results {
        match r.outcome {
            UnlinkOutcome::Unlinked => println!("{GREEN}✓{RESET} {} unlinked", r.display),
            UnlinkOutcome::NotLinked => {
                println!("{DIM}• {} not linked (nothing to do){RESET}", r.display)
            }
            UnlinkOutcome::Failed { ref error } => {
                println!("{RED}✗{RESET} {}: {error}", r.display)
            }
        }
    }
    println!();
    println!(
        "{DIM}Skills stay installed in the canonical dir; use `remove` to delete them.{RESET}"
    );
    println!();
}
