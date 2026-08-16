# agent-skill

[![crates.io](https://img.shields.io/crates/v/agent-skill.svg)](https://crates.io/crates/agent-skill)
[![docs.rs](https://img.shields.io/docsrs/agent-skill.svg)](https://docs.rs/agent-skill)
[![CI](https://github.com/luckie2076/agent-skill/actions/workflows/ci.yml/badge.svg)](https://github.com/luckie2076/agent-skill/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

A minimal, stable **Rust library** for installing and managing AI agent skills,
with an optional command-line interface built on top.

`agent-skill` is **library first**: import it into your Rust project to install,
list, remove, and update `SKILL.md` packages for [Claude Code](https://claude.com/code),
Codex, Cursor, and 70+ other coding agents. A small CLI (`agent-skill`) ships alongside,
implemented as a thin rendering layer over the exact same public API.

> See also: [中文 README](README.zh-CN.md)

## Why a library?

- **Embed skill management into your own tools** — a plugin manager, an agent launcher,
  or a build script can install skills without shelling out to a binary.
- **Pure data, no side effects on stdout** — every API returns structured results and
  surfaces errors via `Result`; it never prints and never calls `process::exit`. You
  decide how to render and when to exit.
- **Injectable context** — `ManagerBuilder` lets you point at any `home`/`config`/`cwd`,
  making tests and sandboxes trivial.

## Getting started

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
agent-skill = "1"
```

Install and list skills with the high-level [`Manager`] facade:

```rust
use agent_skill::{AddRequest, ListRequest, Manager, Result};

fn main() -> Result<()> {
    let manager = Manager::new();

    // Shortcut: install every skill from a source with default options.
    let outcome = manager.add_source("anthropics/skills")?;
    println!("installed {} skill(s)", outcome.installed.len());

    // Full form: install to specific agents (remaining fields default).
    let outcome = manager.add(&AddRequest {
        source: "anthropics/skills".to_string(),
        agents: vec!["*".to_string()],
        ..Default::default()
    })?;
    println!("installed {} skill(s)", outcome.installed.len());

    // List installed skills (serde-serializable; same shape as `list --json`).
    let skills = manager.list(&ListRequest::default())?;
    println!("{skills:?}");
    Ok(())
}
```

## API

### High-level: [`Manager`]

One-stop operations. Each takes a plain request struct and returns a structured outcome.

| Method | Request | Returns |
| ------ | ------- | ------- |
| [`Manager::add`] | [`AddRequest`] | [`AddOutcome`] (installed + failed) |
| [`Manager::add_source`] | `impl Into<String>` | [`AddOutcome`] (installed + failed) |
| [`Manager::list`] | [`ListRequest`] | `Vec<`[`ListedSkill`]`>` (serde-serializable) |
| [`Manager::remove`] | [`RemoveRequest`] | [`RemoveOutcome`] (removed names) |
| [`Manager::update`] | [`UpdateRequest`] | [`UpdateOutcome`] (updated/failed counts) |

Request structs are `Default + Clone` with builder-style field overrides; outcomes are
plain data.

### Injectable context: [`ManagerBuilder`]

```rust
use agent_skill::Manager;

let manager = Manager::builder()
    .home("/tmp/home")
    .config("/tmp/config")
    .cwd("/tmp/project")
    .env_var("CLAUDE_CONFIG_DIR", "/tmp/claude")
    .build();
```

`Manager::new()` is just `Manager::builder().build()` resolved against the real
environment.

### Low-level: `core` primitives

For finer control, the underlying `core` functions are re-exported at the crate root:

- **Source** — [`parse_source`], [`owner_repo`]
- **Discovery** — [`discover_skills`], [`filter_skills`], [`parse_skill_md`]
- **Install** — [`install_skill_for_agent`], [`list_installed_skills`], [`sanitize_name`]
- **Lockfile** — [`read_local_lock`], [`write_local_lock`], [`compute_folder_hash`]
- **Agents** — [`get_agent`], [`detect_installed_agents`], [`Agent`], [`Env`]

### Examples

Run the bundled examples to see real usage:

```bash
cargo run --example manage      # add → list → remove on a scratch dir (no side effects)
cargo run --example add_skill   # install via Manager into your real environment
```

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

## Source formats

The `source` field of [`AddRequest`] (and the CLI `<source>` argument) accepts:

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

## Command-line interface

A small CLI ships on top of the library:

```bash
# Install (from crates.io)
cargo install agent-skill

# Install a skill from a GitHub repo
agent-skill add anthropics/skills

# Install a specific skill, to a specific agent
agent-skill add anthropics/skills@pdf --agent claude-code

# List as machine-readable JSON
agent-skill list --json

# Update everything from its lockfile source
agent-skill update
```

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

## Project structure

```
src/
├── lib.rs              Library root: re-exports Manager + core primitives
├── manager.rs          High-level Manager facade (add/list/remove/update)
├── error.rs            Unified error type and Result alias
├── core/               Domain logic (pure functions, dependency-injectable)
│   ├── source.rs       Source string parsing
│   ├── agents.rs       Agent → skills directory mapping table
│   ├── discover.rs     SKILL.md discovery + frontmatter parsing
│   ├── fetch.rs        git clone / HTTP download / archive extraction
│   ├── install.rs      Install orchestration (canonical + symlink/copy)
│   └── lock.rs         skills-lock.json read/write + content hashing
├── main.rs             Bin entry point (thin CLI over the library)
├── cli.rs              clap command tree (commands, aliases, flags)
└── commands/           CLI rendering layer (arg unpacking + output only)
    ├── add.rs
    ├── remove.rs
    ├── list.rs
    └── update.rs

examples/
├── add_skill.rs        Install a skill via the Manager facade (real usage)
└── manage.rs           add → list → remove lifecycle on a scratch dir

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
cargo run --example manage   # run a library usage example
cargo clippy           # lint
cargo fmt              # format
```

Tests follow the test pyramid: fast, isolated unit tests live inline in `src/` via
`#[cfg(test)]`, while black-box integration tests in `tests/` drive the real CLI through
`assert_cmd`.

## Design choices

The crate intentionally stays minimal and stable:

- **Library first** — the library is the primary interface; the CLI is a thin rendering
  layer over the same public API.
- **Pure data** — the library never prints and never calls `process::exit`; it returns
  structured outcomes and surfaces errors via `Result`.
- **No telemetry** — nothing leaves your machine.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

[`Manager`]: https://docs.rs/agent-skill/latest/agent_skill/struct.Manager.html
[`Manager::add`]: https://docs.rs/agent-skill/latest/agent_skill/struct.Manager.html#method.add
[`Manager::add_source`]: https://docs.rs/agent-skill/latest/agent_skill/struct.Manager.html#method.add_source
[`Manager::list`]: https://docs.rs/agent-skill/latest/agent_skill/struct.Manager.html#method.list
[`Manager::remove`]: https://docs.rs/agent-skill/latest/agent_skill/struct.Manager.html#method.remove
[`Manager::update`]: https://docs.rs/agent-skill/latest/agent_skill/struct.Manager.html#method.update
[`ManagerBuilder`]: https://docs.rs/agent-skill/latest/agent_skill/struct.ManagerBuilder.html
[`AddRequest`]: https://docs.rs/agent-skill/latest/agent_skill/struct.AddRequest.html
[`AddOutcome`]: https://docs.rs/agent-skill/latest/agent_skill/struct.AddOutcome.html
[`ListRequest`]: https://docs.rs/agent-skill/latest/agent_skill/struct.ListRequest.html
[`ListedSkill`]: https://docs.rs/agent-skill/latest/agent_skill/struct.ListedSkill.html
[`RemoveRequest`]: https://docs.rs/agent-skill/latest/agent_skill/struct.RemoveRequest.html
[`RemoveOutcome`]: https://docs.rs/agent-skill/latest/agent_skill/struct.RemoveOutcome.html
[`UpdateRequest`]: https://docs.rs/agent-skill/latest/agent_skill/struct.UpdateRequest.html
[`UpdateOutcome`]: https://docs.rs/agent-skill/latest/agent_skill/struct.UpdateOutcome.html
[`parse_source`]: https://docs.rs/agent-skill/latest/agent_skill/fn.parse_source.html
[`owner_repo`]: https://docs.rs/agent-skill/latest/agent_skill/fn.owner_repo.html
[`discover_skills`]: https://docs.rs/agent-skill/latest/agent_skill/fn.discover_skills.html
[`filter_skills`]: https://docs.rs/agent-skill/latest/agent_skill/fn.filter_skills.html
[`parse_skill_md`]: https://docs.rs/agent-skill/latest/agent_skill/fn.parse_skill_md.html
[`install_skill_for_agent`]: https://docs.rs/agent-skill/latest/agent_skill/fn.install_skill_for_agent.html
[`list_installed_skills`]: https://docs.rs/agent-skill/latest/agent_skill/fn.list_installed_skills.html
[`sanitize_name`]: https://docs.rs/agent-skill/latest/agent_skill/fn.sanitize_name.html
[`read_local_lock`]: https://docs.rs/agent-skill/latest/agent_skill/fn.read_local_lock.html
[`write_local_lock`]: https://docs.rs/agent-skill/latest/agent_skill/fn.write_local_lock.html
[`compute_folder_hash`]: https://docs.rs/agent-skill/latest/agent_skill/fn.compute_folder_hash.html
[`get_agent`]: https://docs.rs/agent-skill/latest/agent_skill/fn.get_agent.html
[`detect_installed_agents`]: https://docs.rs/agent-skill/latest/agent_skill/fn.detect_installed_agents.html
[`Agent`]: https://docs.rs/agent-skill/latest/agent_skill/struct.Agent.html
[`Env`]: https://docs.rs/agent-skill/latest/agent_skill/struct.Env.html
