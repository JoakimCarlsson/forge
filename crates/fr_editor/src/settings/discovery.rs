//! Finding the projects under a folder, for the project picker.

use std::fs;
use std::path::{Path, PathBuf};

use fr_project::Project;

/// A project found in a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredProject {
    /// The folder's name.
    pub name: String,
    /// The project directory.
    pub path: PathBuf,
}

/// The immediate subfolders of `root` that hold a `project.forge`, sorted by
/// name. A missing or unreadable folder gives an empty list.
pub fn discover_projects(root: &Path) -> Vec<DiscoveredProject> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut projects: Vec<DiscoveredProject> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && Project::exists_in(path))
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some(DiscoveredProject { name, path })
        })
        .collect();
    projects.sort_by(|a, b| a.name.cmp(&b.name));
    projects
}
