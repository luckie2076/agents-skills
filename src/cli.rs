//! CLI command tree definition (clap derive).
//!
//! This is the externally exposed interface contract: 4 primary commands + aliases +
//! all flags, centralized in this file for readability.

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "agents-skills",
    about = "A minimal skill installer and manager for AI agents",
    long_about = None,
    disable_version_flag = true
)]
pub struct Cli {
    /// Show version number
    #[arg(short = 'v', long = "version")]
    pub version: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add a skill package (aliases: a, i, install)
    #[command(alias = "a", alias = "i", alias = "install")]
    Add(AddArgs),
    /// Remove installed skills (aliases: rm, r)
    #[command(alias = "rm", alias = "r")]
    Remove(RemoveArgs),
    /// List installed skills (alias: ls)
    #[command(alias = "ls")]
    List(ListArgs),
    /// Update skills to latest versions (aliases: upgrade, check)
    #[command(alias = "upgrade", alias = "check")]
    Update(UpdateArgs),
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Source(s) to install
    #[arg(required = true)]
    pub source: Vec<String>,
    /// Install skill globally (user-level) instead of project-level
    #[arg(short = 'g', long = "global")]
    pub global: bool,
    /// Specify agents to install to (use '*' for all agents)
    #[arg(short = 'a', long = "agent", num_args = 1..)]
    pub agent: Vec<String>,
    /// Specify skill names to install (use '*' for all skills)
    #[arg(short = 's', long = "skill", num_args = 1..)]
    pub skill: Vec<String>,
    /// List available skills in the repository without installing
    #[arg(short = 'l', long = "list")]
    pub list: bool,
    /// Skip confirmation prompts
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
    /// Copy files instead of symlinking to agent directories
    #[arg(long = "copy")]
    pub copy: bool,
    /// Shorthand for --skill '*' --agent '*' -y
    #[arg(long = "all")]
    pub all: bool,
    /// Search all subdirectories even when a root SKILL.md exists
    #[arg(long = "full-depth")]
    pub full_depth: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// Skill names to remove
    pub skills: Vec<String>,
    /// Remove from global scope (~/) instead of project scope
    #[arg(short = 'g', long = "global")]
    pub global: bool,
    /// Remove from specific agents (use '*' for all agents)
    #[arg(short = 'a', long = "agent", num_args = 1..)]
    pub agent: Vec<String>,
    /// Specify skills to remove (use '*' for all skills)
    #[arg(short = 's', long = "skill", num_args = 1..)]
    pub skill: Vec<String>,
    /// Skip confirmation prompts
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
    /// Shorthand for --skill '*' --agent '*' -y
    #[arg(long = "all")]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// List global skills (default: project)
    #[arg(short = 'g', long = "global")]
    pub global: bool,
    /// Filter by specific agents
    #[arg(short = 'a', long = "agent", num_args = 1..)]
    pub agent: Vec<String>,
    /// Output as JSON (machine-readable, no ANSI codes)
    #[arg(long = "json")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Skill names to update
    pub skills: Vec<String>,
    /// Update global skills only
    #[arg(short = 'g', long = "global")]
    pub global: bool,
    /// Update project skills only
    #[arg(short = 'p', long = "project")]
    pub project: bool,
    /// Skip scope prompt (auto-detect: project if in a project, else global)
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

// ============================================================================
// Banner / ASCII logo.
// ============================================================================

pub const RESET: &str = "\x1b[0m";
/// 256-color grayscale, readable on both dark and light backgrounds.
pub const DIM: &str = "\x1b[38;5;102m";
pub const TEXT: &str = "\x1b[38;5;145m";
pub const BOLD: &str = "\x1b[1m";
pub const CYAN: &str = "\x1b[36m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const RED: &str = "\x1b[31m";

const LOGO_LINES: [&str; 6] = [
    "███████╗██╗  ██╗██╗██╗     ██╗     ███████╗",
    "██╔════╝██║ ██╔╝██║██║     ██║     ██╔════╝",
    "███████╗█████╔╝ ██║██║     ██║     ███████╗",
    "╚════██║██╔═██╗ ██║██║     ██║     ╚════██║",
    "███████║██║  ██╗██║███████╗███████╗███████║",
    "╚══════╝╚═╝  ╚═╝╚═╝╚══════╝╚══════╝╚══════╝",
];

const LOGO_GRAYS: [&str; 6] = [
    "\x1b[38;5;250m",
    "\x1b[38;5;248m",
    "\x1b[38;5;245m",
    "\x1b[38;5;243m",
    "\x1b[38;5;240m",
    "\x1b[38;5;238m",
];

pub fn show_logo() {
    println!();
    for (line, gray) in LOGO_LINES.iter().zip(LOGO_GRAYS.iter()) {
        println!("{gray}{line}{RESET}");
    }
}

/// Banner printed when no args are given (experimental commands removed).
pub fn show_banner() {
    show_logo();
    println!();
    println!("{DIM}Agent skill installer and manager{RESET}");
    println!();
    println!(
        "  {DIM}${RESET} {TEXT}agents-skills add {DIM}<package>{RESET}        {DIM}Add a new skill{RESET}"
    );
    println!(
        "  {DIM}${RESET} {TEXT}agents-skills remove{RESET}               {DIM}Remove installed skills{RESET}"
    );
    println!(
        "  {DIM}${RESET} {TEXT}agents-skills list{RESET}                 {DIM}List installed skills{RESET}"
    );
    println!();
    println!(
        "  {DIM}${RESET} {TEXT}agents-skills update{RESET}               {DIM}Update installed skills{RESET}"
    );
    println!();
    println!("{DIM}try:{RESET} agents-skills add anthropics/skills");
    println!();
}
