//! End-to-end tests for the `link` command (link / --status / --unlink) and the directory-link model.

mod common;

use predicates::prelude::*;
use std::path::Path;

use common::TestProject;

#[test]
fn link_creates_relative_dir_symlink() {
    let p = TestProject::new();
    // claude-code links even without .claude/ (historical exception).
    p.skills()
        .args(["link", "claude-code"])
        .assert()
        .success()
        .stdout(predicate::str::contains("linked"));

    let link = p.path().join(".claude/skills");
    assert!(link.is_symlink());
    assert_eq!(
        std::fs::read_link(&link).unwrap(),
        Path::new("../.agents/skills")
    );
}

#[test]
fn link_refuses_content_and_migrate_adopts_it() {
    let p = TestProject::new();
    let existing = p.path().join(".claude/skills/my-skill");
    std::fs::create_dir_all(&existing).unwrap();
    std::fs::write(existing.join("SKILL.md"), "x").unwrap();

    p.skills()
        .args(["link", "claude-code"])
        .assert()
        .success()
        .stdout(predicate::str::contains("migrate"));

    // --migrate moves the skill into the canonical dir and links.
    p.skills()
        .args(["link", "claude-code", "--migrate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("migrated"));

    p.assert_exists(".agents/skills/my-skill/SKILL.md");
    assert!(p.path().join(".claude/skills").is_symlink());
}

#[test]
fn link_refuses_dir_with_only_stray_files() {
    let p = TestProject::new();
    // A real file is not a skill: linking is refused, nothing is touched, and the
    // hint does not (wrongly) point at --migrate.
    std::fs::create_dir_all(p.path().join(".claude/skills")).unwrap();
    std::fs::write(p.path().join(".claude/skills/README.txt"), "x").unwrap();

    p.skills()
        .args(["link", "claude-code"])
        .assert()
        .success()
        .stdout(predicate::str::contains("non-skill files"))
        .stdout(predicate::str::contains("rerun with --migrate").not());

    assert!(p.path().join(".claude/skills/README.txt").exists());
    assert!(!p.path().join(".claude/skills").is_symlink());
}

#[test]
fn unlink_restores_real_dir() {
    let p = TestProject::new();
    p.skills().args(["link", "claude-code"]).assert().success();

    p.skills()
        .args(["link", "--unlink", "claude-code"])
        .assert()
        .success()
        .stdout(predicate::str::contains("unlinked"));

    let dir = p.path().join(".claude/skills");
    assert!(dir.is_dir());
    assert!(!dir.is_symlink());
}

#[test]
fn link_status_prints_installed_agents_and_link_state() {
    let p = TestProject::new();
    p.skills().args(["link", "claude-code"]).assert().success();

    // CodeBuddy is detected via `.codebuddy` in the cwd: installed but not linked.
    std::fs::create_dir_all(p.path().join(".codebuddy")).unwrap();

    p.skills()
        .args(["link", "--status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Agent link status"))
        .stdout(predicate::str::contains("Claude Code"))
        .stdout(predicate::str::contains("(linked)"))
        .stdout(predicate::str::contains("CodeBuddy"))
        .stdout(predicate::str::contains(
            "not linked) — run `agents-skills link codebuddy`",
        ));
}

#[test]
fn link_status_orders_canonical_agents_first() {
    let p = TestProject::new();
    // codex is canonical (universal); claude-code is non-canonical but linked.
    std::fs::create_dir_all(p.path().join(".codex")).unwrap();
    std::fs::create_dir_all(p.path().join(".claude")).unwrap();
    p.skills().args(["link", "claude-code"]).assert().success();

    let out = p
        .skills()
        .env("HOME", p.path())
        .args(["link", "--status"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    // Same order as the library: canonical (universal) agents render first.
    let codex = stdout.find("Codex").expect("codex listed");
    let claude = stdout.find("Claude Code").expect("claude listed");
    assert!(codex < claude);
}

#[test]
fn link_status_marks_universal_agents_as_canonical() {
    let p = TestProject::new();
    // Warp is a universal agent (reads `.agents/skills` natively); pretend it's installed.
    std::fs::create_dir_all(p.path().join(".warp")).unwrap();

    p.skills()
        .env("HOME", p.path())
        .args(["link", "--status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Warp"))
        .stdout(predicate::str::contains("(canonical dir)"));
}

#[test]
fn link_status_conflicts_with_unlink_and_migrate() {
    let p = TestProject::new();

    p.skills()
        .args(["link", "--status", "--unlink"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("cannot be used with"));

    p.skills()
        .args(["link", "--status", "--migrate"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("cannot be used with"));

    p.skills()
        .args(["link", "--unlink", "--migrate"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn add_then_link_ensures_agent_links() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .args(["add", src.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed 1 skill"));

    p.assert_exists(".agents/skills/pdf/SKILL.md");
    // `add` only installs into the canonical dir — no agent link yet.
    assert!(!p.path().join(".claude/skills").is_symlink());

    // `link` exposes the canonical dir to the agent.
    p.skills().args(["link", "claude-code"]).assert().success();
    let link = p.path().join(".claude/skills");
    assert!(link.is_symlink());
    // The skill is visible through the agent link.
    assert!(link.join("pdf/SKILL.md").exists());
}

#[test]
fn remove_skill_disappears_from_linked_agents() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");
    p.skills()
        .args(["add", src.to_str().unwrap()])
        .assert()
        .success();
    p.skills().args(["link", "claude-code"]).assert().success();

    p.skills().args(["remove", "pdf", "-y"]).assert().success();

    p.assert_absent(".agents/skills/pdf");
    // The dir link remains, but the skill is gone (no dead per-skill links).
    assert!(p.path().join(".claude/skills").is_symlink());
    assert!(!p.path().join(".claude/skills/pdf").exists());
}

#[test]
fn link_alias_ln_works() {
    let p = TestProject::new();
    p.skills().args(["ln", "claude-code"]).assert().success();
    assert!(p.path().join(".claude/skills").is_symlink());
}
