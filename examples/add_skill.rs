//! Install a skill using the high-level [`Manager`] facade.
//!
//! Run with:
//!   cargo run --example add_skill -- anthropics/skills
//!   cargo run --example add_skill -- ./path/to/skill
//!
//! Note: this installs into your real environment (project or global scope).
//! Run `agents-skills link` afterwards to expose the skill to your agents.

use agents_skills::{AddRequest, Manager};

fn main() -> agents_skills::Result<()> {
    let source = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "anthropics/skills".to_string());

    let manager = Manager::new();
    let outcome = manager.add(&AddRequest::new(source))?;

    println!("Installed {} skill(s):", outcome.installed.len());
    for s in &outcome.installed {
        println!("  - {}", s.name);
    }
    Ok(())
}
