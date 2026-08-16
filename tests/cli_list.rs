//! End-to-end tests for the `list` command: project/global, JSON, aliases, invalid agent.

mod common;

use predicates::prelude::*;

use common::TestProject;

#[test]
fn list_empty_project_prints_hint() {
    let p = TestProject::new();
    p.skills()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No project skills found."))
        .stdout(predicate::str::contains(
            "Try listing global skills with -g",
        ));
}

#[test]
fn list_json_reports_name_scope_and_source() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .args(["add", src.to_str().unwrap(), "-a", "amp"])
        .assert()
        .success();

    p.skills()
        .args(["list", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"pdf\""))
        .stdout(predicate::str::contains("\"scope\": \"project\""))
        .stdout(predicate::str::contains("\"source\":"));
}

#[test]
fn list_plain_prints_skill_agents_and_source() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .args(["add", src.to_str().unwrap(), "-a", "amp"])
        .assert()
        .success();

    p.skills()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Project Skills"))
        .stdout(predicate::str::contains("pdf"))
        .stdout(predicate::str::contains("Source:"));
}

#[test]
fn list_supports_alias_ls() {
    let p = TestProject::new();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .args(["add", src.to_str().unwrap(), "-a", "amp"])
        .assert()
        .success();

    p.skills()
        .arg("ls")
        .assert()
        .success()
        .stdout(predicate::str::contains("pdf"));
}

#[test]
fn list_global_scope_with_g() {
    let p = TestProject::new();
    let home = p.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let src = p.write_skill_source("my-skill", "pdf");

    p.skills()
        .env("HOME", &home)
        .args(["add", src.to_str().unwrap(), "-g", "-a", "amp"])
        .assert()
        .success();

    p.skills()
        .env("HOME", &home)
        .args(["list", "-g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Global Skills"))
        .stdout(predicate::str::contains("pdf"));
}

#[test]
fn list_invalid_agent_exits_nonzero() {
    let p = TestProject::new();
    p.skills()
        .args(["list", "-a", "not-a-real-agent"])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("Invalid agents"));
}
