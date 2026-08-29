//! agent: manage agents' skills dirs and their link state relative to the canonical dir.
//!
//! Three modes, selected by a required flag — mirroring [`AgentRequest`] on the library side:
//! - `--link`: connect agents' skills dirs via directory-level symlinks (existing
//!   content is parked in a backup slot; `--migrate` moves skills into the canonical dir)
//! - `--status`: show which agents are linked ([`Manager::agent_status`])
//! - `--unlink`: disconnect agents' skills dirs and restore parked content
//!
//! Renders outcomes; no business logic lives here.

use crate::cli::{BOLD, DIM, GREEN, RESET, YELLOW};
use crate::commands::{fail_agents, render_link_result, shorten_path};
use agents_skills::error::Result;
use agents_skills::{AgentOutcome, AgentRequest, LinkOutcome, Manager};

/// Run the `agent` command; `--status` reads only, `--unlink` disconnects, otherwise link.
pub fn run(manager: &Manager, args: crate::cli::AgentArgs) -> Result<()> {
    if args.status {
        render_status(manager, args.global);
        return Ok(());
    }
    let req = AgentRequest {
        agents: args.agents,
        global: args.global,
        unlink: args.unlink,
        migrate: args.migrate,
    };
    let outcome = match manager.agent(&req) {
        Ok(o) => o,
        Err(e) => return fail_agents(e),
    };
    render_link(&outcome, args.unlink, manager.env());
    Ok(())
}

fn render_status(manager: &Manager, global: bool) {
    let scope = if global { "global" } else { "project" };
    println!("{BOLD}Agent link status ({scope}){RESET}");
    println!();
    // Order comes from the library: canonical agents first, others keep table order.
    for s in manager.agent_status(global) {
        if s.canonical {
            println!(
                "  {DIM}•{RESET} {} {DIM}({}) — canonical{RESET}",
                s.display, s.name
            );
        } else if s.linked {
            println!(
                "  {GREEN}✓{RESET} {} {DIM}({}) — linked{RESET}",
                s.display, s.name
            );
        } else {
            println!(
                "  {YELLOW}!{RESET} {} {DIM}({}) — not linked{RESET}",
                s.display, s.name
            );
            if !s.internal_skills.is_empty() {
                println!(
                    "      {DIM}private skills: {}{RESET}",
                    s.internal_skills.join(", ")
                );
            }
            if !s.internal_others.is_empty() {
                println!(
                    "      {DIM}other files: {}{RESET}",
                    s.internal_others.join(", ")
                );
            }
            if let Some(b) = &s.pending_backup {
                println!(
                    "      {DIM}backup parked at {} ({}) — unlink restores{RESET}",
                    shorten_path(&b.path, manager.env()),
                    b.items.join(", ")
                );
            }
        }
    }
    println!();
}

fn render_link(outcome: &AgentOutcome, unlink: bool, env: &agents_skills::Env) {
    let scope = if outcome.global { "global" } else { "project" };
    if unlink {
        println!("{DIM}Unlinking agents from the {scope} canonical skills dir{RESET}");
    } else {
        println!("{DIM}Linking agents to the {scope} canonical skills dir{RESET}");
    }
    println!();
    for r in &outcome.results {
        render_link_result(r, env);
    }
    if unlink {
        println!();
        println!(
            "{DIM}Skills moved into the canonical dir stay there; use `remove` to delete them.{RESET}"
        );
    } else {
        // The --migrate hint only helps when parked content actually holds skills;
        // slots with only non-skill files can't be migrated.
        let needs_migrate = outcome.results.iter().any(|r| match &r.outcome {
            LinkOutcome::Linked { parked_skills, .. } => !parked_skills.is_empty(),
            _ => false,
        });
        if needs_migrate {
            println!();
            println!(
                "{DIM}Parked skills can be moved into the canonical dir: rerun with --migrate.{RESET}"
            );
        }
    }
    println!();
}
