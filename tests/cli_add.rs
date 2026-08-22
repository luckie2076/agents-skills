//! End-to-end tests for the `add` command: local install, aliases, global, --list, --full-depth.

mod common;

use predicates::prelude::*;

use common::{TestProject, skill_md};

#[test]
fn add_local_path_installs_to_canonical() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .arg("add")
        .arg(&src)
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed 1 skill"));

    p.assert_exists(".agents/skills/pdf/SKILL.md");
    p.assert_exists("skills-lock.json");
    assert!(p.read("skills-lock.json").contains("\"pdf\""));
}

#[test]
fn add_supports_full_word_alias_install() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    // `install` is an alias of `add`.
    p.skills().arg("install").arg(&src).assert().success();

    p.assert_exists(".agents/skills/pdf/SKILL.md");
}

#[test]
fn add_supports_short_alias_a() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .args(["a", src.to_str().unwrap()])
        .assert()
        .success();

    p.assert_exists(".agents/skills/pdf/SKILL.md");
}

#[test]
fn add_list_flag_prints_without_installing() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .arg("add")
        .arg(&src)
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available Skills"))
        .stdout(predicate::str::contains("pdf"));

    p.assert_absent(".agents/skills/pdf");
}

#[test]
fn add_missing_local_path_exits_nonzero() {
    let p = TestProject::new();
    let missing = p.path().join("does-not-exist");

    p.skills()
        .args(["add", missing.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("Local path does not exist"));
}

#[test]
fn add_global_installs_to_home() {
    let p = TestProject::new();
    let home = p.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .env("HOME", &home)
        .args(["add", src.to_str().unwrap(), "-g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed 1 skill"));

    assert!(home.join(".agents/skills/pdf/SKILL.md").exists());
    p.assert_absent(".agents/skills/pdf");
}

#[test]
fn add_full_depth_discovers_deep_skills() {
    let p = TestProject::new();
    let deep = p.path().join("skills/a/b/pdf");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("SKILL.md"), skill_md("pdf-deep")).unwrap();

    p.skills()
        .arg("add")
        .arg(p.path())
        .args(["--full-depth", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pdf-deep"));
}
