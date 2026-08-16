# Choosing the right approach

This guide explains the *recommended* way to use `agent-skill` for common scenarios.
It complements the API reference — use it when you're deciding *which* knobs to turn,
not *what* a function does.

## Pick the right source format

| You want to… | Use | Example |
| ------------ | --- | ------- |
| Install a well-known public skill set | GitHub shorthand | `anthropics/skills` |
| Pin a specific version/branch | GitHub URL with `/tree/<ref>` | `https://github.com/owner/repo/tree/v1.2` |
| Install a single skill from a repo | `@skill` shorthand | `anthropics/skills@pdf` |
| Ship a prebuilt archive | Direct download URL | `https://example.com/skills.zip` |
| Develop a skill locally | Local path | `./my-skill` |

**Best practice:** pin to a ref (tag or commit) when reproducibility matters — e.g. in a
CI pipeline or a team lockfile. Use the shorthand `owner/repo` only for personal,
track-latest workflows.

## Choose project vs. global scope

- **Project scope** (default) installs into `./.agents/skills` and records the lockfile at
  `./skills-lock.json` *inside the project*. This is the right choice for team-shared skills:
  commit the lockfile so everyone gets the same versions.
- **Global scope** (`--global` / `global: true`) installs into `~/.agents/skills` with the
  lockfile at `~/.agents/.skill-lock.json`. Use it for personal, cross-project skills.

**Best practice:** default to project scope for anything the repo depends on; reserve global
scope for personal utility skills.

## Symlink vs. copy

- **Symlink** (default) writes the skill to the canonical `.agents/skills` dir once, then
  symlinks it into each agent directory. One source of truth, and `update` re-points it in
  place. Recommended for local development.
- **Copy** (`--copy` / `copy: true`) duplicates the files into each agent dir (skipping
  `metadata.json`, `.git`, `__pycache__`, `__pypackages__`). Use it when the target agent
  sandbox can't follow symlinks, or you want a fully self-contained snapshot.

**Best practice:** symlink for day-to-day; copy when distributing to a constrained runtime.

## Scope auto-detection on `update`

`update` uses [`Scope::Auto`] by default: it targets project scope if the project has a
lockfile or a `.agents/skills` dir, otherwise global. Set the scope explicitly when a repo
could exist in both contexts.

## Handle errors, don't print

The library is pure data: every operation returns a [`Result`] and never writes to stdout or
calls `process::exit`. Map outcomes to your own UI:

- Inspect [`AddOutcome::failed`] / [`UpdateOutcome::failures`] for per-skill failures that
  did *not* abort the whole operation.
- A returned `Err` means the operation couldn't proceed at all (bad source, invalid agent
  name, transport failure). Match on [`SkillsError`] variants to decide messaging.

## Write hermetic tests

Point the manager at a scratch directory so tests never touch the real home or network:

```rust,ignore
use agent_skill::Manager;

let tmp = tempfile::TempDir::new().unwrap();
let manager = Manager::builder()
    .home(tmp.path().join("home"))
    .config(tmp.path().join("config"))
    .cwd(tmp.path().join("project"))
    .build();
```

See the bundled `examples/` for complete, runnable lifecycles.
