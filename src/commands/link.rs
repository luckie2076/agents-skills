//! link: manage agents' skills dirs and their link state relative to the canonical dir.
//!
//! Three modes, selected by flag:
//! - default: connect agents' skills dirs via directory-level symlinks ([`Manager::link`])
//! - `--status`: show which agents are linked ([`Manager::link_status`])
//! - `--unlink`: disconnect agents' skills dirs ([`Manager::unlink`])
//!
//! Renders outcomes; no business logic lives here.

use crate::cli::{BOLD, DIM, GREEN, RED, RESET, YELLOW};
use crate::commands::{fail_agents, render_link_result};
use agents_skills::error::Result;
use agents_skills::{
    LinkManagerOutcome, LinkOutcome, LinkRequest, Manager, UnlinkManagerOutcome, UnlinkOutcome,
    UnlinkRequest,
};

pub fn run(manager: &Manager, args: crate::cli::LinkArgs) -> Result<()> {
    if args.status {
        render_status(manager, args.global);
    } else if args.unlink {
        run_unlink(manager, args)?;
    } else {
        run_link(manager, args)?;
    }
    Ok(())
}

fn run_link(manager: &Manager, args: crate::cli::LinkArgs) -> Result<()> {
    let req = LinkRequest {
        agents: args.agents,
        global: args.global,
        migrate: args.migrate,
    };
    let outcome = match manager.link(&req) {
        Ok(o) => o,
        Err(e) => return fail_agents(e),
    };
    render_link(&outcome);
    Ok(())
}

fn run_unlink(manager: &Manager, args: crate::cli::LinkArgs) -> Result<()> {
    let req = UnlinkRequest {
        agents: args.agents,
        global: args.global,
    };
    let outcome = match manager.unlink(&req) {
        Ok(o) => o,
        Err(e) => return fail_agents(e),
    };
    render_unlink(&outcome);
    Ok(())
}

fn render_status(manager: &Manager, global: bool) {
    let scope = if global { "global" } else { "project" };
    println!("{BOLD}Agent link status ({scope}){RESET}");
    println!();
    for s in manager.link_status(global) {
        if s.linked {
            println!("  {DIM}•{RESET} {} {DIM}(canonical dir){RESET}", s.display);
        } else {
            let hint = if global {
                format!("run `agents-skills link {} -g`", s.name)
            } else {
                format!("run `agents-skills link {}`", s.name)
            };
            println!(
                "  {YELLOW}!{RESET} {} {DIM}(not linked) — {hint}{RESET}",
                s.display
            );
        }
    }
    println!();
}

fn render_link(outcome: &LinkManagerOutcome) {
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

fn render_unlink(outcome: &UnlinkManagerOutcome) {
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
