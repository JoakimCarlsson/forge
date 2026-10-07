//! Planning how to build and run a project's game, without running anything.
//!
//! The game reads `--project <dir>` and `--scene <path>` (relative to the asset
//! root) in `fr_engine::parse_run_options`, and takes the working directory as
//! the project when `--project` is absent. The plan passes both, so the
//! game finds its project whatever directory it runs in.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

use fr_project::{BuildStep, Project, build_steps};

use crate::document::absolute_path;
use crate::error::AuthoringError;

/// A process to start: the program, its arguments and its working directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchCommand {
    /// The program: the game's absolute path, or `cargo`.
    pub program: PathBuf,
    /// The arguments.
    pub args: Vec<String>,
    /// The directory the process runs in.
    pub working_directory: PathBuf,
}

impl LaunchCommand {
    /// The process as a [`Command`], not yet spawned.
    pub fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.working_directory);
        command
    }

    /// The command as one line for a log, with arguments quoted when they
    /// contain spaces.
    pub fn display(&self) -> String {
        let mut line = self.program.display().to_string();
        for argument in &self.args {
            line.push(' ');
            if argument.contains(' ') {
                line.push('"');
                line.push_str(argument);
                line.push('"');
            } else {
                line.push_str(argument);
            }
        }
        format!("(in {}) {line}", self.working_directory.display())
    }
}

/// How to build a project's game and start it on a scene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPlan {
    /// The steps that build the game, from `fr_project::build_steps`.
    pub build: Vec<BuildStep>,
    /// The process that runs the game.
    pub launch: LaunchCommand,
}

/// The steps that build the project's game.
pub fn build_plan(project: &Project) -> Vec<BuildStep> {
    build_steps(project)
}

/// A scene path as asset-root-relative text with forward slashes. An absolute
/// path must lie under the asset root; a relative one is taken as relative to
/// it and must not climb out.
///
/// # Errors
///
/// [`AuthoringError::OutsideAssetRoot`] when the scene is outside the root.
fn scene_argument(asset_root: &Path, scene: &Path) -> Result<String, AuthoringError> {
    let root = absolute_path(asset_root);
    let outside = || AuthoringError::OutsideAssetRoot {
        path: scene.to_path_buf(),
        asset_root: root.clone(),
    };
    let relative = if scene.is_absolute() {
        absolute_path(scene)
            .strip_prefix(&root)
            .map_err(|_| outside())?
            .to_path_buf()
    } else {
        scene.to_path_buf()
    };
    let mut parts: Vec<String> = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return Err(outside()),
        }
    }
    Ok(parts.join("/"))
}

/// The plan for running a project on a scene given as an absolute path or a path
/// relative to the asset root.
///
/// With a game executable in `project.forge` the program is its absolute path.
/// Without one it is `cargo run --release --manifest-path <root>/Cargo.toml --
/// ...`, so cargo builds the project at its root while the game itself runs in
/// the asset root. The working directory is the asset root in both cases, and
/// the game's arguments are `--project <root> --scene <relative path>`.
///
/// # Errors
///
/// [`AuthoringError::OutsideAssetRoot`] when the scene is outside the asset
/// root.
pub fn launch_plan(project: &Project, scene: &Path) -> Result<LaunchPlan, AuthoringError> {
    let asset_root = project.asset_root();
    let game_arguments = vec![
        "--project".to_owned(),
        project.root().display().to_string(),
        "--scene".to_owned(),
        scene_argument(&asset_root, scene)?,
    ];
    let launch = match project.game_binary() {
        Some(program) => LaunchCommand {
            program,
            args: game_arguments,
            working_directory: asset_root,
        },
        None => {
            let manifest = project.root().join("Cargo.toml");
            let mut args = vec![
                "run".to_owned(),
                "--release".to_owned(),
                "--manifest-path".to_owned(),
                manifest.display().to_string(),
                "--".to_owned(),
            ];
            args.extend(game_arguments);
            LaunchCommand {
                program: PathBuf::from("cargo"),
                args,
                working_directory: asset_root,
            }
        }
    };
    Ok(LaunchPlan {
        build: build_plan(project),
        launch,
    })
}
