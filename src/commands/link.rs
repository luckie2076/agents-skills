//! link: connect agents' skills dirs to the canonical dir via directory-level symlinks.
//!
//! Renders the [`Manager::link`] outcome; no business logic lives here.

use crate::cli::{DIM, RESET};
use crate::commands::{fail_agents, render_link_result};
use agents_skills::error::Result;
use agents_skills::{LinkManagerOutcome, LinkOutcome, LinkRequest, Manager};

pub fn run(manager: &Manager, args: crate::cli::LinkArgs) -> Result<()> {
    let req = LinkRequest {
        agents: args.agents,
        global: args.global,
        migrate: args.migrate,
    };
    let outcome = match manager.link(&req) {
        Ok(o) => o,
        Err(e) => return fail_agents(e),
    };
    render(&outcome);
    Ok(())
}

fn render(outcome: &LinkManagerOutcome) {
    let scope = if outcome.global { "global" } else { "project" };
    println!("{DIM}Linking agents to the {scope} canonical skills dir{RESET}");
    println!();
    for r in &outcome.results {
        render_link_result(r);
    }

    let refused = outcome
        .results
        .iter()
        .any(|r| matches!(r.outcome, LinkOutcome::Refused { .. }));
    if refused {
        println!();
        println!(
            "{DIM}Rerun with --migrate to move existing skills into the canonical dir.{RESET}"
        );
    }
    println!();
}
