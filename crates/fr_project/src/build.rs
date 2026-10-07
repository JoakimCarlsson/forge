//! The cargo steps that build a project's game.

use std::path::PathBuf;
use std::process::Command;

use crate::error::ProjectError;
use crate::project::Project;

/// One process of a project build with its progress and failure messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildStep {
    /// The program to run.
    pub executable: String,
    /// The arguments it is given.
    pub arguments: Vec<String>,
    /// The directory it runs in.
    pub working_directory: PathBuf,
    /// What the step is doing, in the present participle.
    pub description: String,
}

impl BuildStep {
    /// Runs the step and waits for it.
    ///
    /// # Errors
    ///
    /// [`ProjectError::Build`] when the program cannot start or exits with a
    /// failure status.
    pub fn run(&self) -> Result<(), ProjectError> {
        let failure = |reason: String| ProjectError::Build {
            description: self.description.clone(),
            reason,
        };
        let status = Command::new(&self.executable)
            .args(&self.arguments)
            .current_dir(&self.working_directory)
            .status()
            .map_err(|error| failure(error.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(failure(format!("{} exited with {status}", self.executable)))
        }
    }
}

/// The steps that build a project's game: a release build of its Cargo project.
pub fn build_steps(project: &Project) -> Vec<BuildStep> {
    vec![BuildStep {
        executable: "cargo".to_owned(),
        arguments: vec!["build".to_owned(), "--release".to_owned()],
        working_directory: project.root().to_path_buf(),
        description: "Compiling".to_owned(),
    }]
}
