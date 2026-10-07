//! The per-user editor settings in the config directory.

use std::path::{Path, PathBuf};

use super::ini::{all_values, first_value, format_ini, parse_ini, read_optional, write_text};

/// The folder in the user's config directory that holds the editor's files.
const CONFIG_FOLDER: &str = "forge-editor";

/// The settings file in the config folder.
const SETTINGS_FILE: &str = "editor.ini";

/// The dock layout file in the config folder.
const LAYOUT_FILE: &str = "layout.txt";

/// The folder inside the documents directory where new projects go.
const PROJECTS_FOLDER: &str = "forge";

/// How many recent projects are remembered.
pub const MAX_RECENT_PROJECTS: usize = 10;

/// The editor's per-user settings: the last and recent projects, the window
/// size and where new projects are created. They live in a folder of their own
/// in the user's config directory and never in a project.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// The folder the files live in.
    directory: PathBuf,
    /// The project open when the editor last closed.
    last_project: Option<PathBuf>,
    /// The recent projects, the most recent first.
    recent_projects: Vec<PathBuf>,
    /// The window size in pixels.
    window_size: Option<(u32, u32)>,
    /// The folder new projects are created in and the picker lists.
    projects_root: PathBuf,
}

/// `~/Documents/forge`, or a `projects` folder in the config folder when the
/// system has no documents directory.
fn default_projects_root(directory: &Path) -> PathBuf {
    dirs::document_dir().map_or_else(
        || directory.join("projects"),
        |documents| documents.join(PROJECTS_FOLDER),
    )
}

/// The existing paths among the values, without repeats, at most
/// [`MAX_RECENT_PROJECTS`].
fn existing_unique<'a>(values: impl Iterator<Item = &'a str>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for value in values {
        let path = PathBuf::from(value);
        if path.is_dir() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths.truncate(MAX_RECENT_PROJECTS);
    paths
}

/// Parses `WIDTHxHEIGHT`.
fn parse_window_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.split_once('x')?;
    Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
}

impl Settings {
    /// The defaults for a config folder, reading nothing.
    pub fn in_directory(directory: impl Into<PathBuf>) -> Self {
        let directory = directory.into();
        Self {
            projects_root: default_projects_root(&directory),
            directory,
            last_project: None,
            recent_projects: Vec::new(),
            window_size: None,
        }
    }

    /// Loads the settings of the current user from `forge-editor` in the config
    /// directory; a missing file gives the defaults.
    ///
    /// # Errors
    ///
    /// The reason, as text, when the system has no config directory or the file
    /// exists and cannot be read.
    pub fn load() -> Result<Self, String> {
        let base =
            dirs::config_dir().ok_or_else(|| "the system has no config directory".to_owned())?;
        Self::load_from(base.join(CONFIG_FOLDER))
    }

    /// Loads the settings from a config folder; a missing file gives the
    /// defaults.
    ///
    /// # Errors
    ///
    /// The reason, as text, when the file exists and cannot be read.
    pub fn load_from(directory: impl Into<PathBuf>) -> Result<Self, String> {
        let mut settings = Self::in_directory(directory);
        let Some(text) = read_optional(&settings.settings_path())? else {
            return Ok(settings);
        };
        let entries = parse_ini(&text);
        settings.last_project = first_value(&entries, "last_project").map(PathBuf::from);
        settings.recent_projects = existing_unique(all_values(&entries, "recent_project"));
        settings.window_size = first_value(&entries, "window_size").and_then(parse_window_size);
        if let Some(root) = first_value(&entries, "projects_root").filter(|root| !root.is_empty()) {
            settings.projects_root = PathBuf::from(root);
        }
        Ok(settings)
    }

    /// Writes the settings atomically, creating the folder when needed.
    ///
    /// # Errors
    ///
    /// The reason, as text, when the file cannot be written.
    pub fn save(&self) -> Result<(), String> {
        let mut entries = Vec::new();
        if let Some(project) = &self.last_project {
            entries.push(("last_project", project.display().to_string()));
        }
        for project in &self.recent_projects {
            entries.push(("recent_project", project.display().to_string()));
        }
        if let Some((width, height)) = self.window_size {
            entries.push(("window_size", format!("{width}x{height}")));
        }
        entries.push(("projects_root", self.projects_root.display().to_string()));
        write_text(&self.settings_path(), &format_ini(entries))
    }

    /// The folder the editor's files live in.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The settings file.
    pub fn settings_path(&self) -> PathBuf {
        self.directory.join(SETTINGS_FILE)
    }

    /// The dock layout file.
    pub fn layout_path(&self) -> PathBuf {
        self.directory.join(LAYOUT_FILE)
    }

    /// Puts a project first among the recent ones and makes it the last one.
    /// Repeats are dropped and only [`MAX_RECENT_PROJECTS`] are kept.
    pub fn remember_project(&mut self, path: &Path) {
        self.recent_projects.retain(|recent| recent != path);
        self.recent_projects.insert(0, path.to_path_buf());
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        self.last_project = Some(path.to_path_buf());
    }

    /// The project open last, when its folder still exists.
    pub fn last_project(&self) -> Option<&Path> {
        self.last_project.as_deref().filter(|path| path.is_dir())
    }

    /// The recent projects whose folders still exist, the most recent first.
    pub fn recent_projects(&self) -> Vec<&Path> {
        self.recent_projects
            .iter()
            .map(PathBuf::as_path)
            .filter(|path| path.is_dir())
            .collect()
    }

    /// The window size in pixels, when one was saved.
    pub fn window_size(&self) -> Option<(u32, u32)> {
        self.window_size
    }

    /// Remembers the window size.
    pub fn set_window_size(&mut self, size: (u32, u32)) {
        self.window_size = Some(size);
    }

    /// The folder new projects are created in and the picker lists.
    pub fn projects_root(&self) -> &Path {
        &self.projects_root
    }

    /// Changes the folder new projects are created in.
    pub fn set_projects_root(&mut self, root: impl Into<PathBuf>) {
        self.projects_root = root.into();
    }
}
