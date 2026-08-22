//! End-to-end tests for the `remove` command: single removal, aliases, --all, no-args, nonexistent skill.

mod common;

use predicates::prelude::*;

use common::TestProject;

#[test]
fn remove_deletes_installed_skill_and_lock_entry() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .args(["add", src.to_str().unwrap()])
        .assert()
        .success();
    p.assert_exists(".agents/skills/pdf");

    p.skills()
        .args(["remove", "pdf"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully removed 1 skill"));

    p.assert_absent(".agents/skills/pdf");
    assert!(
        !p.read("skills-lock.json").contains("\"pdf\""),
        "lock should drop skill"
    );
}

#[test]
fn remove_supports_alias_rm() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .args(["add", src.to_str().unwrap()])
        .assert()
        .success();

    p.skills().args(["rm", "pdf"]).assert().success();

    p.assert_absent(".agents/skills/pdf");
}

#[test]
fn remove_all_flag_removes_everything() {
    let p = TestProject::new();
    let a = p.write_skill_source("skill-a", "pdf");
    let b = p.write_skill_source("skill-b", "docx");

    p.skills()
        .args(["add", a.to_str().unwrap()])
        .assert()
        .success();
    p.skills()
        .args(["add", b.to_str().unwrap()])
        .assert()
        .success();

    p.skills().args(["remove", "--all"]).assert().success();

    p.assert_absent(".agents/skills/pdf");
    p.assert_absent(".agents/skills/docx");
}

#[test]
fn remove_without_args_prints_installed_list() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .args(["add", src.to_str().unwrap()])
        .assert()
        .success();

    p.skills()
        .arg("remove")
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed skills"))
        .stdout(predicate::str::contains("pdf"));

    p.assert_exists(".agents/skills/pdf");
}

#[test]
fn remove_nonexistent_prints_no_match() {
    let p = TestProject::new();
    p.skills()
        .args(["remove", "ghost"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No matching skills found"));
}
