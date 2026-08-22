//! End-to-end tests for the `link` / `unlink` commands and the directory-link model.

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
fn unlink_restores_real_dir() {
    let p = TestProject::new();
    p.skills().args(["link", "claude-code"]).assert().success();

    p.skills()
        .args(["unlink", "claude-code"])
        .assert()
        .success()
        .stdout(predicate::str::contains("unlinked"));

    let dir = p.path().join(".claude/skills");
    assert!(dir.is_dir());
    assert!(!dir.is_symlink());
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
