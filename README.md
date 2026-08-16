# agent-skill

[![crates.io](https://img.shields.io/crates/v/agent-skill.svg)](https://crates.io/crates/agent-skill)
[![CI](https://github.com/luckie2076/agent-skill/actions/workflows/ci.yml/badge.svg)](https://github.com/luckie2076/agent-skill/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A minimal, stable, and easy-to-understand skill installer and manager for AI agents,
written in Rust.

Install and manage AI agent **skills** — reusable, versioned `SKILL.md` packages — for
[Claude Code](https://claude.com/code), Codex, Cursor, and 70+ other coding agents.

The interface is deliberately small: 4 primary commands, a lockfile for reproducible
updates, and install locations shared by the major coding agents. The implementation is
a clean, idiomatic Rust program built on mature crates.

> See also: [中文 README](README.zh-CN.md)

## Features

- **Install from anywhere** — local paths, GitHub repos/URLs, GitLab, SSH/git URLs, and
  arbitrary HTTPS endpoints (well-known discovery or direct download).
- **70+ agents** — static directory-mapping table, data-driven and dependency-injectable
  for testability.
- **Project and global scopes** — install to `.agents/skills` (project) or `~/.agents/skills`
  (global) with symlink or copy mode.
- **Lockfile** — `skills-lock.json` records the source and a SHA-256 content hash for every
  installed skill, enabling reproducible `update`.
- **Skill discovery** — priority container dirs (`skills/`, `.curated/`, `.experimental/`,
  `.system/`) with shallow-shadowing-deep resolution.
- **Cross-platform** — macOS, Linux, and Windows (directory symlinks on Windows, `git2` for
  transport-agnostic cloning).

## Installation

```bash
# From crates.io (recommended)
cargo install agent-skill

# From source
git clone https://github.com/luckie2076/agent-skill
cd agent-skill
cargo install --path .

# Verify
agent-skill --version   # 1.5.22
```

## Quick start

```bash
# Install a skill from a GitHub repo (shorthand)
agent-skill add anthropics/skills

# Install a specific skill from a repo
agent-skill add anthropics/skills@pdf

# Install from a local path, to a specific agent
agent-skill add ./my-skill --agent claude-code

# List installed skills (project scope)
agent-skill list

# List as machine-readable JSON
agent-skill list --json

# Update everything from its lockfile source
agent-skill update
```

## Using as a library

`agent-skill` also ships as a Rust library. Add it to your `Cargo.toml`:

```toml
[dependencies]
agent-skill = "1"
```

Use the high-level `Manager` facade:

```rust
use agent_skill::{AddRequest, ListRequest, Manager, Result};

fn main() -> Result<()> {
    let manager = Manager::new();

    // Install every skill from a GitHub repo to all detected agents.
    manager.add(&AddRequest {
        source: "anthropics/skills".to_string(),
        agents: vec!["*".to_string()],
        ..Default::default()
    })?;

    // List installed skills (serde-serializable; same shape as `list --json`).
    let skills = manager.list(&ListRequest::default())?;
    println!("{skills:?}");
    Ok(())
}
```

For finer control, the low-level `core` primitives are re-exported at the crate
root (`parse_source`, `discover_skills`, `install_skill_for_agent`,
`read_local_lock`, …).

## Commands

| Command | Aliases | Description |
| ------- | ------- | ----------- |
| `add` | `a`, `i`, `install` | Install skill packages from a source |
| `remove` | `rm`, `r` | Remove installed skills |
| `list` | `ls` | List installed skills |
| `update` | `upgrade`, `check` | Update skills to their latest versions |

### `add`

```
agent-skill add <source> [options]

Options:
  -g, --global        Install globally (user-level) instead of project-level
  -a, --agent <a>...  Agents to install to ('*' for all)
  -s, --skill <s>...  Skill names to install ('*' for all)
  -l, --list          List available skills without installing
      --copy          Copy files instead of symlinking
      --all           Shorthand for --skill '*' --agent '*' -y
      --full-depth    Search all subdirectories even with a root SKILL.md
  -y, --yes           Skip confirmation prompts
```

### `remove`

```
agent-skill remove [skills...] [options]

Options:
  -g, --global        Remove from global scope instead of project scope
  -a, --agent <a>...  Remove from specific agents ('*' for all)
  -s, --skill <s>...  Skills to remove ('*' for all)
      --all           Shorthand for --skill '*' --agent '*' -y
  -y, --yes           Skip confirmation prompts
```

### `list`

```
agent-skill list [options]

Options:
  -g, --global        List global skills (default: project)
  -a, --agent <a>...  Filter by specific agents
      --json          Output as JSON (machine-readable, no ANSI codes)
```

### `update`

```
agent-skill update [skills...] [options]

Options:
  -g, --global        Update global skills only
  -p, --project       Update project skills only
  -y, --yes           Skip the scope prompt (auto-detect)
```

## Source formats

The `<source>` argument accepts:

| Format | Example |
| ------ | ------- |
| Local path | `./my-skill`, `/abs/path/skill` |
| GitHub shorthand | `owner/repo`, `owner/repo@skill`, `owner/repo/subpath` |
| GitHub URL | `https://github.com/owner/repo`, `.../tree/main/skills` |
| GitLab URL | `https://gitlab.com/group/repo`, `.../-/tree/main/skills` |
| SSH / git URL | `git@github.com:owner/repo.git` |
| HTTPS (well-known) | `https://example.com/skills` (discovery → download fallback) |
| HTTPS (download) | `.../skill.zip`, `.../skill.tar.gz`, raw `SKILL.md` |

## Install locations

- **Project scope** — `./.agents/skills/<name>` (canonical), symlinked into each agent's
  project skills directory.
- **Global scope** — `~/.agents/skills/<name>` (canonical), plus each agent's user-level
  skills directory.

## Project structure

```
src/
├── main.rs             Bin entry point: banner, version, subcommand dispatch
├── lib.rs              Library root: re-exports Manager + core primitives
├── manager.rs          High-level Manager facade (add/list/remove/update)
├── cli.rs              clap command tree (commands, aliases, flags)
├── error.rs            Unified error type and Result alias
├── commands/           CLI rendering layer (thin orchestration)
│   ├── add.rs
│   ├── remove.rs
│   ├── list.rs
│   └── update.rs
└── core/               Domain logic (CLI-agnostic, dependency-injectable)
    ├── source.rs       Source string parsing
    ├── agents.rs       Agent → skills directory mapping table
    ├── discover.rs     SKILL.md discovery + frontmatter parsing
    ├── fetch.rs        git clone / HTTP download / archive extraction
    ├── install.rs      Install orchestration (canonical + symlink/copy)
    └── lock.rs         skills-lock.json read/write + content hashing

tests/
├── common/mod.rs       Shared integration-test fixtures
├── lib_api.rs          Library API integration tests
├── cli_add.rs
├── cli_remove.rs
├── cli_list.rs
└── cli_version.rs
```

## Development

```bash
cargo build            # build
cargo test             # run all tests (61 unit + 26 integration)
cargo clippy           # lint
cargo fmt              # format
```

Tests follow the test pyramid: fast, isolated unit tests live inline in `src/` via
`#[cfg(test)]`, while black-box integration tests in `tests/` drive the real CLI through
`assert_cmd`.

## Design choices

The tool intentionally stays minimal and stable:

- **Four commands** — `add`, `remove`, `list`, and `update` cover the whole install/manage
  lifecycle; other common commands (scaffolding a `SKILL.md`, generating a prompt without
  installing, searching a central skill registry) are out of scope.
- **Non-interactive by default** — every command is scriptable; confirmation prompts are
  removed (the `-y` flag is kept as a no-op for CLI compatibility).
- **No telemetry** — nothing leaves your machine.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
