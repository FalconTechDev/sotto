use std::fs;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn valid_project_task_file_can_be_validated_without_opening_the_store() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("project");
    let data = temp.path().join("sotto-data");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("sotto.toml"),
        "project_id = \"11111111-1111-1111-1111-111111111111\"\nproject = \"example\"\nenvironment = \"dev\"\n",
    )
    .unwrap();
    fs::write(
        project.join("sotto.tasks.toml"),
        r#"version = 1

[[task]]
name = "test"
description = "Run the test suite"
executable = "cargo"
args = ["test"]
environments = ["dev"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sotto"))
        .arg("task")
        .arg("validate")
        .current_dir(&project)
        .env("SOTTO_DATA_DIR", &data)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "valid: 1 task\n");
    assert!(
        !data.exists(),
        "validation must not initialise local secret state"
    );
}

#[test]
fn invalid_task_file_reports_its_path_without_opening_the_store() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("project");
    let data = temp.path().join("sotto-data");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("sotto.toml"),
        "project_id = \"11111111-1111-1111-1111-111111111111\"\nproject = \"example\"\nenvironment = \"dev\"\n",
    )
    .unwrap();
    let manifest = project.join("sotto.tasks.toml");
    fs::write(
        &manifest,
        "version = 2\n\n[[task]]\nname = \"test\"\ndescription = \"Run tests\"\nexecutable = \"cargo\"\nenvironments = [\"dev\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sotto"))
        .args(["task", "validate"])
        .current_dir(&project)
        .env("SOTTO_DATA_DIR", &data)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&manifest.display().to_string()), "{stderr}");
    assert!(
        stderr.contains("unsupported task file version 2"),
        "{stderr}"
    );
    assert!(
        !data.exists(),
        "validation must not initialise local secret state"
    );
}

#[test]
fn malformed_task_file_does_not_echo_source_text() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("sotto.toml"),
        "project_id = \"11111111-1111-1111-1111-111111111111\"\nproject = \"example\"\nenvironment = \"dev\"\n",
    )
    .unwrap();
    fs::write(
        project.join("sotto.tasks.toml"),
        "version = 1\n\n[[task]]\nname = \"test\"\ndescription = \"do_not_echo_this_secret\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sotto"))
        .args(["task", "validate"])
        .current_dir(&project)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("sotto.tasks.toml:"), "{stderr}");
    assert!(!stderr.contains("do_not_echo_this_secret"), "{stderr}");
}

#[test]
fn missing_task_file_has_creation_guidance_without_opening_the_store() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("project");
    let data = temp.path().join("sotto-data");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("sotto.toml"),
        "project_id = \"11111111-1111-1111-1111-111111111111\"\nproject = \"example\"\nenvironment = \"dev\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_sotto"))
        .args(["task", "validate"])
        .current_dir(&project)
        .env("SOTTO_DATA_DIR", &data)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("sotto.tasks.toml"), "{stderr}");
    assert!(stderr.contains("create this file"), "{stderr}");
    assert!(
        !data.exists(),
        "validation must not initialise local secret state"
    );
}
