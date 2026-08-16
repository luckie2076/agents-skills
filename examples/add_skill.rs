//! Install a skill using the high-level [`Manager`] facade.
//!
//! Run with:
//!   cargo run --example add_skill -- anthropics/skills
//!   cargo run --example add_skill -- ./path/to/skill --agent claude-code
//!
//! Note: this installs into your real environment (project or global scope).

use agent_skill::{AddRequest, Manager};

fn main() -> agent_skill::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // First positional argument is the source; anything after `--agent` is a filter.
    let source = args
        .first()
        .cloned()
        .unwrap_or_else(|| "anthropics/skills".to_string());
    let agents = match args.iter().position(|a| a == "--agent") {
        Some(i) => args[i + 1..].to_vec(),
        None => Vec::new(),
    };

    let manager = Manager::new();
    let outcome = manager.add(&AddRequest {
        source,
        agents,
        ..Default::default()
    })?;

    println!("Installed {} skill(s):", outcome.installed.len());
    for s in &outcome.installed {
        println!("  - {}", s.name);
    }
    Ok(())
}
