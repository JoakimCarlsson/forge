//! An opened project: its directory, its settings and where its assets are.

use std::path::{Path, PathBuf};

use fr_document::{AssetIndex, read_text_file, write_file};

use crate::error::ProjectError;
use crate::settings::{ProjectSettings, SETTINGS_FILE, read_project_text, write_project_text};

/// A project directory with its `project.forge` read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// The directory holding `project.forge`.
    root: PathBuf,
    /// The settings read from it.
    settings: ProjectSettings,
}

impl Project {
    /// A project over a directory and settings, which are not written.
    pub fn new(root: PathBuf, settings: ProjectSettings) -> Self {
        Self { root, settings }
    }

    /// Opens the project in a directory.
    ///
    /// # Errors
    ///
    /// When the directory holds no `project.forge` or it cannot be read or
    /// understood.
    pub fn open(root: &Path) -> Result<Self, ProjectError> {
        let path = root.join(SETTINGS_FILE);
        if !path.is_file() {
            return Err(ProjectError::NotAProject {
                root: root.to_path_buf(),
            });
        }
        let text = read_text_file(&path)?;
        let name = root
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        Ok(Self {
            root: root.to_path_buf(),
            settings: read_project_text(&text, &name, &path)?,
        })
    }

    /// Whether a directory holds a project.
    pub fn exists_in(root: &Path) -> bool {
        root.join(SETTINGS_FILE).is_file()
    }

    /// Writes the settings back to `project.forge`.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn save(&self) -> Result<(), ProjectError> {
        write_file(
            &self.root.join(SETTINGS_FILE),
            write_project_text(&self.settings).as_bytes(),
        )?;
        Ok(())
    }

    /// The directory holding `project.forge`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The settings read from `project.forge`.
    pub fn settings(&self) -> &ProjectSettings {
        &self.settings
    }

    /// The settings, to change before a [`Project::save`].
    pub fn settings_mut(&mut self) -> &mut ProjectSettings {
        &mut self.settings
    }

    /// The directory assets live in.
    pub fn asset_root(&self) -> PathBuf {
        self.root.join(&self.settings.asset_root)
    }

    /// The scene the game starts in, relative to the asset root; none when the
    /// project names none.
    pub fn startup_scene(&self) -> Option<&str> {
        Some(self.settings.startup_scene.as_str()).filter(|scene| !scene.is_empty())
    }

    /// The absolute path of the game executable; none when the project names
    /// none.
    pub fn game_binary(&self) -> Option<PathBuf> {
        if self.settings.game_executable.is_empty() {
            return None;
        }
        let joined = self.root.join(&self.settings.game_executable);
        Some(std::path::absolute(&joined).unwrap_or(joined))
    }

    /// The identity and path of every asset of the project.
    pub fn asset_index(&self) -> AssetIndex {
        AssetIndex::build(&self.asset_root())
    }
}
