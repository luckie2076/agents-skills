//! Integration tests for the library API: proving the lib works independently of the CLI,
//! driven entirely through `ManagerBuilder` + request structs (no `assert_cmd`).

use std::path::{Path, PathBuf};

use agents_skills::{AddRequest, ListRequest, Manager, RemoveRequest, SkillsError, UpdateRequest};

fn write_skill_source(root: &Path, rel_dir: &str, name: &str) -> PathBuf {
    let dir = root.join(rel_dir);
    std::fs::create_dir_all(&dir).unwrap();
    let md = format!("---\nname: {name}\ndescription: does {name}\n---\n\n# {name}\n");
    std::fs::write(dir.join("SKILL.md"), md).unwrap();
    dir
}

#[test]
fn lib_add_list_remove_roundtrip() {
    let tmp = tempfile::TempDir::new().unwrap();
    let cwd = tmp.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();

    let manager = Manager::builder()
        .home(tmp.path().join("home"))
        .config(tmp.path().join("config"))
        .cwd(cwd.clone())
        .build();

    // Add a local skill to a specific agent.
    let src = write_skill_source(tmp.path(), "src", "pdf");
    let outcome = manager
        .add(&AddRequest {
            source: src.display().to_string(),
            agents: vec!["amp".to_string()],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(outcome.installed.len(), 1);
    assert!(outcome.failed.is_empty());
    assert!(cwd.join(".agents/skills/pdf/SKILL.md").exists());

    // List finds it, with scope and lock metadata.
    let listed = manager.list(&ListRequest::default()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "pdf");
    assert_eq!(listed[0].scope, "project");
    assert!(listed[0].source.is_some());

    // Remove it.
    let removed = manager
        .remove(&RemoveRequest {
            skills: vec!["pdf".to_string()],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(removed.removed, vec!["pdf".to_string()]);
    assert!(!cwd.join(".agents/skills/pdf").exists());
}

#[test]
fn lib_add_global_uses_home() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&cwd).unwrap();

    let manager = Manager::builder()
        .home(home.clone())
        .config(tmp.path().join("config"))
        .cwd(cwd.clone())
        .build();

    let src = write_skill_source(tmp.path(), "src", "pdf");
    let outcome = manager
        .add(&AddRequest {
            source: src.display().to_string(),
            global: true,
            agents: vec!["amp".to_string()],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(outcome.installed.len(), 1);
    assert!(home.join(".agents/skills/pdf/SKILL.md").exists());
    assert!(!cwd.join(".agents/skills/pdf").exists());
}

#[test]
fn lib_invalid_agent_returns_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manager = Manager::builder().cwd(tmp.path().join("project")).build();
    let src = write_skill_source(tmp.path(), "src", "pdf");

    let err = manager
        .add(&AddRequest {
            source: src.display().to_string(),
            agents: vec!["not-an-agent".to_string()],
            ..Default::default()
        })
        .unwrap_err();
    assert!(matches!(err, SkillsError::InvalidAgents(_)));
}

#[test]
fn lib_missing_local_path_returns_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manager = Manager::builder().cwd(tmp.path().join("project")).build();

    let err = manager
        .add(&AddRequest {
            source: tmp.path().join("nope").display().to_string(),
            ..Default::default()
        })
        .unwrap_err();
    assert!(err.to_string().contains("Local path does not exist"));
}

#[test]
fn lib_list_json_shape() {
    let tmp = tempfile::TempDir::new().unwrap();
    let cwd = tmp.path().join("project");
    std::fs::create_dir_all(&cwd).unwrap();

    let manager = Manager::builder().cwd(cwd).build();
    let src = write_skill_source(tmp.path(), "src", "pdf");
    manager
        .add(&AddRequest {
            source: src.display().to_string(),
            agents: vec!["amp".to_string()],
            ..Default::default()
        })
        .unwrap();

    let listed = manager.list(&ListRequest::default()).unwrap();
    let json = serde_json::to_string_pretty(&listed).unwrap();
    assert!(json.contains("\"name\": \"pdf\""));
    assert!(json.contains("\"scope\": \"project\""));
    assert!(json.contains("\"source\":"));
}

#[test]
fn lib_update_empty_is_noop() {
    let tmp = tempfile::TempDir::new().unwrap();
    let manager = Manager::builder()
        .home(tmp.path().join("home"))
        .cwd(tmp.path().join("project"))
        .build();

    let outcome = manager.update(&UpdateRequest::default()).unwrap();
    assert_eq!(outcome.updated, 0);
    assert_eq!(outcome.failed, 0);
}
