//! list: list installed skills (project/global, `--json`, `-a` agent filter).

use crate::cli::{BOLD, CYAN, DIM, GREEN, ListArgs, RESET, YELLOW};
use crate::commands::{fail_agents, shorten_path};
use agents_skills::core::agents::Env;
use agents_skills::error::Result;
use agents_skills::{ListRequest, ListedSkill, Manager};

pub fn run(manager: &Manager, args: ListArgs) -> Result<()> {
    let req = ListRequest {
        global: args.global,
        agents: args.agent.clone(),
    };
    let listed = match manager.list(&req) {
        Ok(l) => l,
        Err(e) => return fail_agents(e),
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&listed)?);
        return Ok(());
    }

    let scope_label = if args.global { "Global" } else { "Project" };
    if listed.is_empty() {
        println!(
            "{DIM}No {} skills found.{RESET}",
            scope_label.to_lowercase()
        );
        if args.global {
            println!("{DIM}Try listing project skills without -g{RESET}");
        } else {
            println!("{DIM}Try listing global skills with -g{RESET}");
        }
        return Ok(());
    }

    println!("{BOLD}{} Skills{RESET}", scope_label);
    println!();
    for skill in &listed {
        print_skill(skill, manager.env());
    }
    println!();
    Ok(())
}

fn print_skill(skill: &ListedSkill, env: &Env) {
    let short = shorten_path(&skill.path, env);
    let source_label = skill.source.clone().unwrap_or_else(|| "local".to_string());
    let status = if skill.enabled {
        format!("{GREEN}enabled{RESET}")
    } else {
        format!("{YELLOW}disabled{RESET}")
    };
    println!(
        "{CYAN}{}{RESET} {DIM}{}{RESET} [{status}]",
        skill.name, short
    );
    println!("  {DIM}Source:{RESET} {source_label}");
}
