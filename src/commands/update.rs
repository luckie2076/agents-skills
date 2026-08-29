//! update: reinstall from the source recorded in the lock. Renders the [`Manager::update`] outcome.

use crate::cli::{DIM, GREEN, RED, RESET, TEXT, UpdateArgs};
use agents_skills::error::Result;
use agents_skills::{Manager, Scope, UpdateOutcome, UpdateRequest};

pub fn run(manager: &Manager, args: UpdateArgs) -> Result<()> {
    let scope = if args.project.is_some() {
        Scope::Project
    } else {
        Scope::Global
    };
    let req = UpdateRequest {
        scope,
        skills: args.skills.clone(),
    };

    if !args.skills.is_empty() {
        println!("{TEXT}Updating {}…{RESET}", args.skills.join(", "));
    } else {
        println!("{TEXT}Checking for skill updates…{RESET}");
    }
    println!();

    let outcome = manager.update(&req)?;
    render(&req, &outcome);
    Ok(())
}

fn render(req: &UpdateRequest, outcome: &UpdateOutcome) {
    let scope = if outcome.global { "global" } else { "project" };

    if outcome.updated == 0 && outcome.failed == 0 {
        if !req.skills.is_empty() {
            println!(
                "{DIM}No installed skills found matching: {}{RESET}",
                req.skills.join(", ")
            );
        } else {
            println!("{DIM}No {scope} skills to update.{RESET}");
        }
        return;
    }

    for name in &outcome.updated_names {
        println!("{GREEN}✓{RESET} Updated {name}");
    }
    for f in &outcome.failures {
        println!("{RED}✗{RESET} {f}");
    }

    println!();
    if outcome.updated > 0 {
        println!("{GREEN}✓ Updated {} skill(s){RESET}", outcome.updated);
    }
    if outcome.failed > 0 {
        println!("{RED}Failed to update {} skill(s){RESET}", outcome.failed);
        std::process::exit(1);
    }
    println!();
}
