//! Project-shared, secret-free declarations for named agent tasks.
//!
//! This module only parses and validates declarations. It never resolves secrets or starts a
//! process; later approval and execution layers consume the validated task representation.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::error::{Error, Result};

pub const TASKS_FILE: &str = "sotto.tasks.toml";
pub const TASKS_VERSION: u32 = 1;

const MAX_TASKS: usize = 128;
const MAX_ENVIRONMENTS_PER_TASK: usize = 64;
const MAX_ARGUMENTS: usize = 128;
const MAX_TEXT_LENGTH: usize = 4096;
const MAX_TASK_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 256;
const MAX_ENVIRONMENT_NAME_LENGTH: usize = 128;

/// A validated manifest stored at the Sotto project root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskManifest {
    pub version: u32,
    #[serde(rename = "task")]
    pub tasks: Vec<TaskDeclaration>,
}

/// One fixed executable and argument vector that a developer may later approve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDeclaration {
    pub name: String,
    pub description: String,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub environments: Vec<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl TaskManifest {
    /// Find the nearest Sotto project, then load its task manifest from that project root.
    pub fn discover(start: &Path) -> Result<(PathBuf, Self)> {
        let (_, project_root) = Config::discover(start)?;
        let manifest = Self::load_from(&project_root)?;
        Ok((project_root, manifest))
    }

    /// Load and validate `sotto.tasks.toml` from a known Sotto project root.
    pub fn load_from(project_root: &Path) -> Result<Self> {
        let path = project_root.join(TASKS_FILE);
        let text = std::fs::read_to_string(&path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound(format!(
                "{} (create this file to declare agent tasks)",
                path.display()
            )),
            _ => Error::Io(format!("reading {}: {error}", path.display())),
        })?;

        let manifest: Self = toml::from_str(&text)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        manifest.validate().map_err(|error| match error {
            Error::Config(message) => Error::Config(format!("{}: {message}", path.display())),
            other => other,
        })?;
        Ok(manifest)
    }

    /// Parse and validate TOML, including the file path in configuration errors.
    pub fn parse(path: &Path, text: &str) -> Result<Self> {
        let manifest: Self = toml::from_str(text)
            .map_err(|error| Error::Config(format!("{}: {error}", path.display())))?;
        manifest.validate().map_err(|error| match error {
            Error::Config(message) => Error::Config(format!("{}: {message}", path.display())),
            other => other,
        })?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<()> {
        if self.version != TASKS_VERSION {
            return Err(Error::Config(format!(
                "unsupported task file version {}; this CLI supports version {TASKS_VERSION}",
                self.version
            )));
        }
        if self.tasks.is_empty() {
            return Err(Error::Config("at least one task must be declared".into()));
        }
        if self.tasks.len() > MAX_TASKS {
            return Err(Error::Config(format!(
                "task file may declare at most {MAX_TASKS} tasks"
            )));
        }

        let mut task_names = HashSet::with_capacity(self.tasks.len());
        for (index, task) in self.tasks.iter().enumerate() {
            validate_task(task, index)?;
            if !task_names.insert(task.name.as_str()) {
                return Err(task_error(index, "name", "duplicates an earlier task name"));
            }
        }
        Ok(())
    }

    /// Fingerprint one named task together with the manifest schema version.
    pub fn fingerprint_for(&self, name: &str) -> Result<String> {
        let task = self
            .tasks
            .iter()
            .find(|task| task.name == name)
            .ok_or_else(|| Error::NotFound(format!("agent task `{name}`")))?;
        let canonical = serde_json::to_vec(&VersionedTask {
            version: self.version,
            task,
        })
        .map_err(|error| {
            Error::Config(format!("could not canonicalise task declaration: {error}"))
        })?;
        Ok(format!("{:x}", Sha256::digest(canonical)))
    }
}

impl TaskDeclaration {
    /// Resolve a declared executable path against the project root, or mark a bare name for
    /// lookup on `PATH` when the task is eventually launched.
    pub fn executable_target(&self, project_root: &Path) -> ExecutableTarget {
        let executable = Path::new(&self.executable);
        let is_bare_name = !executable.is_absolute()
            && executable.components().count() == 1
            && executable.file_name().is_some();

        if is_bare_name {
            ExecutableTarget::SearchPath(self.executable.clone())
        } else if executable.is_absolute() {
            ExecutableTarget::Path(executable.to_path_buf())
        } else {
            ExecutableTarget::Path(project_root.join(executable))
        }
    }

    /// Resolve the declared working directory against the project root without shell or
    /// environment-variable expansion. The default is the project root.
    pub fn working_directory(&self, project_root: &Path) -> PathBuf {
        match &self.working_directory {
            Some(directory) => {
                let path = Path::new(directory);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    project_root.join(path)
                }
            }
            None => project_root.to_path_buf(),
        }
    }
}

#[derive(Serialize)]
struct VersionedTask<'a> {
    version: u32,
    task: &'a TaskDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutableTarget {
    /// Resolve this executable name using the process `PATH` when launching.
    SearchPath(String),
    /// Launch this path directly.
    Path(PathBuf),
}

fn validate_task(task: &TaskDeclaration, index: usize) -> Result<()> {
    if task.name.is_empty()
        || task.name.len() > MAX_TASK_NAME_LENGTH
        || !task.name.is_ascii()
        || !task.name.as_bytes()[0].is_ascii_lowercase()
            && !task.name.as_bytes()[0].is_ascii_digit()
        || !task
            .name
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !task.name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
    {
        return Err(task_error(
            index,
            "name",
            "must be 1-64 lowercase ASCII letters, digits, hyphens or underscores, starting and ending with a letter or digit",
        ));
    }
    if task.description.trim().is_empty()
        || task.description.chars().count() > MAX_DESCRIPTION_LENGTH
        || task.description.chars().any(char::is_control)
    {
        return Err(task_error(
            index,
            "description",
            "must be non-empty, at most 256 characters and contain no control characters",
        ));
    }
    validate_text_field(&task.executable, index, "executable", false)?;
    if task.args.len() > MAX_ARGUMENTS {
        return Err(task_error(
            index,
            "args",
            "may contain at most 128 arguments",
        ));
    }
    for argument in &task.args {
        validate_text_field(argument, index, "args", true)?;
    }
    if task.environments.is_empty() {
        return Err(task_error(
            index,
            "environments",
            "must list at least one Sotto environment",
        ));
    }
    if task.environments.len() > MAX_ENVIRONMENTS_PER_TASK {
        return Err(task_error(
            index,
            "environments",
            "may list at most 64 environments",
        ));
    }
    let mut environments = HashSet::with_capacity(task.environments.len());
    for environment in &task.environments {
        if environment.trim().is_empty()
            || environment.trim() != environment
            || environment.chars().count() > MAX_ENVIRONMENT_NAME_LENGTH
            || environment.chars().any(char::is_control)
        {
            return Err(task_error(
                index,
                "environments",
                "each environment name must be trimmed, non-empty, at most 128 characters and contain no control characters",
            ));
        }
        if !environments.insert(environment.as_str()) {
            return Err(task_error(
                index,
                "environments",
                "contains a duplicate environment name",
            ));
        }
    }
    if let Some(directory) = &task.working_directory {
        validate_text_field(directory, index, "working_directory", false)?;
    }
    if task.timeout_seconds == Some(0) {
        return Err(task_error(
            index,
            "timeout_seconds",
            "must be greater than zero when specified",
        ));
    }
    Ok(())
}

fn validate_text_field(value: &str, index: usize, field: &str, allow_empty: bool) -> Result<()> {
    if (!allow_empty && value.trim().is_empty())
        || value.len() > MAX_TEXT_LENGTH
        || value.contains('\0')
    {
        return Err(task_error(
            index,
            field,
            "must be non-empty when required, at most 4096 bytes and contain no NUL character",
        ));
    }
    Ok(())
}

fn task_error(index: usize, field: &str, message: &str) -> Error {
    Error::Config(format!("task {} field `{field}` {message}", index + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
version = 1

[[task]]
name = "test"
description = "Run the test suite"
executable = "cargo"
args = ["test"]
environments = ["dev"]
"#;

    #[test]
    fn parses_a_valid_task_manifest() {
        let manifest = TaskManifest::parse(Path::new("sotto.tasks.toml"), VALID).unwrap();

        assert_eq!(manifest.version, TASKS_VERSION);
        assert_eq!(manifest.tasks.len(), 1);
        assert_eq!(manifest.tasks[0].name, "test");
        assert_eq!(manifest.tasks[0].executable, "cargo");
        assert_eq!(manifest.tasks[0].args, ["test"]);
        assert_eq!(manifest.tasks[0].environments, ["dev"]);
    }

    #[test]
    fn rejects_unsupported_versions_and_unknown_fields() {
        let unsupported = VALID.replace("version = 1", "version = 2");
        assert!(TaskManifest::parse(Path::new("tasks.toml"), &unsupported).is_err());

        let unknown_field = VALID.replace("description =", "unexpected = true\ndescription =");
        assert!(TaskManifest::parse(Path::new("tasks.toml"), &unknown_field).is_err());
    }

    #[test]
    fn rejects_duplicate_tasks_and_duplicate_environments() {
        let duplicate_task = format!(
            "{VALID}\n\n[[task]]\nname = \"test\"\ndescription = \"Another test\"\nexecutable = \"cargo\"\nargs = []\nenvironments = [\"stage\"]\n"
        );
        assert!(TaskManifest::parse(Path::new("tasks.toml"), &duplicate_task).is_err());

        let duplicate_environment = VALID.replace("[\"dev\"]", "[\"dev\", \"dev\"]");
        assert!(TaskManifest::parse(Path::new("tasks.toml"), &duplicate_environment).is_err());
    }

    #[test]
    fn rejects_invalid_names_empty_environments_and_zero_timeout() {
        for invalid in [
            VALID.replace("name = \"test\"", "name = \"Bad Name\""),
            VALID.replace("environments = [\"dev\"]", "environments = []"),
            VALID.replace(
                "environments = [\"dev\"]",
                "environments = [\"dev\"]\ntimeout_seconds = 0",
            ),
        ] {
            assert!(TaskManifest::parse(Path::new("tasks.toml"), &invalid).is_err());
        }
    }

    #[test]
    fn task_fingerprint_ignores_toml_formatting_but_tracks_execution_changes() {
        let original = TaskManifest::parse(Path::new("tasks.toml"), VALID).unwrap();
        let reformatted = TaskManifest::parse(
            Path::new("other.toml"),
            &VALID.replace("args = [\"test\"]", "args=[\"test\"] # formatting only"),
        )
        .unwrap();
        assert_eq!(
            original.fingerprint_for("test").unwrap(),
            reformatted.fingerprint_for("test").unwrap()
        );

        let changed = TaskManifest::parse(
            Path::new("tasks.toml"),
            &VALID.replace("args = [\"test\"]", "args = [\"test\", \"--all\"]"),
        )
        .unwrap();
        assert_ne!(
            original.fingerprint_for("test").unwrap(),
            changed.fingerprint_for("test").unwrap()
        );
    }

    #[test]
    fn resolves_bare_executables_and_project_relative_paths_consistently() {
        let manifest = TaskManifest::parse(
            Path::new("tasks.toml"),
            &VALID.replace(
                "args = [\"test\"]",
                "args = [\"test\"]\nworking_directory = \"packages/api\"",
            ),
        )
        .unwrap();
        let root = Path::new("/work/project");
        let task = &manifest.tasks[0];

        assert_eq!(
            task.executable_target(root),
            ExecutableTarget::SearchPath("cargo".into())
        );
        assert_eq!(task.working_directory(root), root.join("packages/api"));

        let relative_executable = TaskManifest::parse(
            Path::new("tasks.toml"),
            &VALID.replace("executable = \"cargo\"", "executable = \"scripts/test.sh\""),
        )
        .unwrap();
        assert_eq!(
            relative_executable.tasks[0].executable_target(root),
            ExecutableTarget::Path(root.join("scripts/test.sh"))
        );
    }

    #[test]
    fn discovers_task_file_at_the_nearest_project_root() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let nested = project.join("src/nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            project.join(crate::config::CONFIG_FILE),
            "project_id = \"11111111-1111-1111-1111-111111111111\"\nproject = \"example\"\nenvironment = \"dev\"\n",
        )
        .unwrap();
        std::fs::write(project.join(TASKS_FILE), VALID).unwrap();

        let (root, manifest) = TaskManifest::discover(&nested).unwrap();
        assert_eq!(root, project);
        assert_eq!(manifest.tasks[0].name, "test");
    }
}
