//! add: install skills (local path / GitHub / git / download).
//!
//! Renders the [`Manager::add`] outcome; no business logic lives here.

use std::collections::HashSet;
use std::path::Path;

use crate::cli::{AddArgs, BOLD, CYAN, DIM, GREEN, RED, RESET, YELLOW};
use crate::commands::fail_agents;
use agent_skill::error::Result;
use agent_skill::{AddOutcome, AddRequest, Env, Manager, SkillsError, Source, SourceType};

pub fn run(manager: &Manager, args: AddArgs) -> Result<()> {
    let (skills, agents) = if args.all {
        (vec!["*".to_string()], vec!["*".to_string()])
    } else {
        (args.skill.clone(), args.agent.clone())
    };
    let req = AddRequest {
        source: args.source[0].clone(),
        global: args.global,
        agents,
        skills,
        list_only: args.list,
        copy: args.copy,
        full_depth: args.full_depth,
    };

    let outcome = match manager.add(&req) {
        Ok(o) => o,
        Err(e) => return fail_add(e),
    };
    render(manager.env(), &req, &outcome);
    Ok(())
}

fn fail_add(e: SkillsError) -> Result<()> {
    match e {
        SkillsError::Message(msg) => {
            println!("{RED}{msg}{RESET}");
            std::process::exit(1);
        }
        other => fail_agents(other),
    }
}

fn render(env: &Env, req: &AddRequest, outcome: &AddOutcome) {
    print_source(&outcome.source);
    println!(
        "Found {GREEN}{}{RESET} skill{}",
        outcome.skills.len(),
        if outcome.skills.len() > 1 { "s" } else { "" }
    );

    if outcome.list_only {
        println!();
        println!("{BOLD}Available Skills{RESET}");
        for skill in &outcome.skills {
            println!("  {CYAN}{}{RESET}", skill.name);
            println!("    {DIM}{}{RESET}", skill.description);
        }
        println!();
        println!("Use --skill <name> to install specific skills");
        return;
    }

    // Selection message.
    if req.skills.iter().any(|s| s == "*") {
        println!("Installing all {} skills", outcome.skills.len());
    } else if !req.skills.is_empty() {
        if outcome.selected.is_empty() {
            println!(
                "{RED}No matching skills found for: {}{RESET}",
                req.skills.join(", ")
            );
            println!("Available skills:");
            for s in &outcome.skills {
                println!("  - {}", s.name);
            }
            std::process::exit(1);
        }
        println!(
            "Selected {} skill{}: {}",
            outcome.selected.len(),
            if outcome.selected.len() != 1 { "s" } else { "" },
            outcome
                .selected
                .iter()
                .map(|s| CYAN.to_string() + &s.name + RESET)
                .collect::<Vec<_>>()
                .join(", ")
        );
    } else if outcome.skills.len() == 1 {
        let first = &outcome.skills[0];
        println!("Skill: {CYAN}{}{RESET}", first.name);
        println!("{DIM}{}{RESET}", first.description);
    } else {
        println!("Installing all {} skills", outcome.skills.len());
    }

    // Target agents message (only for auto-detection).
    if req.agents.is_empty() {
        if outcome.target_agents.len() == 1 {
            println!("Installing to: {CYAN}{}{RESET}", outcome.target_agents[0]);
        } else {
            println!(
                "Installing to: {}",
                outcome
                    .target_agents
                    .iter()
                    .map(|a| CYAN.to_string() + a + RESET)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    // Results.
    if !outcome.installed.is_empty() {
        let count = outcome
            .installed
            .iter()
            .map(|i| i.name.as_str())
            .collect::<HashSet<_>>()
            .len();
        println!();
        for s in &outcome.installed {
            if let Some(canonical) = &s.canonical_path {
                println!("{GREEN}✓{RESET} {}", shorten_path(canonical, env));
            } else {
                println!("{GREEN}✓{RESET} {}", s.name);
            }
        }
        println!();
        println!(
            "{GREEN}Installed {count} skill{}{RESET}",
            if count != 1 { "s" } else { "" }
        );
    }
    if !outcome.failed.is_empty() {
        println!();
        println!("{RED}Failed to install {}{RESET}", outcome.failed.len());
        for f in &outcome.failed {
            println!(
                "  {RED}✗{RESET} {} → {}: {DIM}{}{RESET}",
                f.skill, f.agent, f.error
            );
        }
    }
    println!();
    println!(
        "{GREEN}Done!{RESET}{DIM}  Review skills before use; they run with full agent permissions.{RESET}"
    );
}

fn print_source(parsed: &Source) {
    let main = match parsed.ty {
        SourceType::Local => parsed
            .local_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        _ => parsed.url.clone(),
    };
    let mut line = format!("Source: {main}");
    if let Some(r) = &parsed.r#ref {
        line.push_str(&format!(" @ {YELLOW}{r}{RESET}"));
    }
    if let Some(sp) = &parsed.subpath {
        line.push_str(&format!(" ({sp})"));
    }
    if let Some(sf) = &parsed.skill_filter {
        line.push_str(&format!(" {DIM}@{RESET}{CYAN}{sf}{RESET}"));
    }
    println!("{line}");
}

fn shorten_path(path: &Path, env: &Env) -> String {
    let full = path.to_string_lossy();
    let home_s = env.home.to_string_lossy();
    let cwd_s = env.cwd.to_string_lossy();
    if full == home_s {
        return "~".to_string();
    }
    if let Some(rest) = full.strip_prefix(&*home_s) {
        if rest.starts_with('/') || rest.starts_with('\\') {
            return format!("~{rest}");
        }
    }
    if full == cwd_s {
        return ".".to_string();
    }
    if let Some(rest) = full.strip_prefix(&*cwd_s) {
        if rest.starts_with('/') || rest.starts_with('\\') {
            return format!(".{rest}");
        }
    }
    full.to_string()
}
