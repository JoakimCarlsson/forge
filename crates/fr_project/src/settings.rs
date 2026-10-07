//! The `project.forge` file: one `key = value` setting per line.

use std::path::Path;

use crate::error::ProjectError;

/// The name of the settings file at a project's root.
pub const SETTINGS_FILE: &str = "project.forge";

/// The settings a project file holds; paths are text, as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSettings {
    /// The project name.
    pub name: String,
    /// The directory assets live in, relative to the project root.
    pub asset_root: String,
    /// The game executable relative to the project root; empty for none.
    pub game_executable: String,
    /// The scene the game starts in, relative to the asset root; empty for none.
    pub startup_scene: String,
}

impl Default for ProjectSettings {
    /// An untitled project whose assets sit in the project root.
    fn default() -> Self {
        Self {
            name: "Untitled Project".to_owned(),
            asset_root: ".".to_owned(),
            game_executable: String::new(),
            startup_scene: String::new(),
        }
    }
}

/// Parses a project file; the name defaults to the name of the project
/// directory. Blank lines and lines starting with `#` are skipped and unknown
/// keys are ignored.
///
/// # Errors
///
/// [`ProjectError::Settings`] naming the line that is not `key = value`.
pub fn read_project_text(
    source: &str,
    directory_name: &str,
    path: &Path,
) -> Result<ProjectSettings, ProjectError> {
    let mut settings = ProjectSettings {
        name: directory_name.to_owned(),
        ..ProjectSettings::default()
    };
    for (number, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(ProjectError::Settings {
                path: path.to_path_buf(),
                reason: format!("line {} is not `key = value`", number + 1),
            });
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "name" if !value.is_empty() => value.clone_into(&mut settings.name),
            "asset_root" => {
                settings.asset_root = if value.is_empty() { "." } else { value }.to_owned();
            }
            "game_executable" => value.clone_into(&mut settings.game_executable),
            "startup_scene" => value.clone_into(&mut settings.startup_scene),
            _ => {}
        }
    }
    Ok(settings)
}

/// Serialises a project file with its keys in alphabetical order.
pub fn write_project_text(settings: &ProjectSettings) -> String {
    format!(
        "asset_root = {}\ngame_executable = {}\nname = {}\nstartup_scene = {}\n",
        settings.asset_root, settings.game_executable, settings.name, settings.startup_scene,
    )
}
