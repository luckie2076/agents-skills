//! End-to-end tests for version metadata output.

mod common;

use predicates::prelude::*;

use common::TestProject;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn version_prints_pure_semver() {
    TestProject::new()
        .skills()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::eq(format!("{VERSION}\n")));
}

#[test]
fn short_version_flag_prints_same() {
    TestProject::new()
        .skills()
        .arg("-v")
        .assert()
        .success()
        .stdout(predicate::eq(format!("{VERSION}\n")));
}
