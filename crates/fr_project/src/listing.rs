//! The listing of a project's asset root: every folder and file in tree order
//! with the kind and identity of each asset.

use std::fs;
use std::path::Path;

use fr_document::{AssetIndex, AssetKind, Guid, meta_path, relative_text};

use crate::project::Project;

/// One folder or file of the asset root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectEntry {
    /// The path relative to the asset root, with forward slashes.
    pub path: String,
    /// How many folders deep the entry is; zero at the root.
    pub depth: usize,
    /// Whether the entry is a folder.
    pub is_directory: bool,
    /// What the file is; [`AssetKind::Other`] for a folder.
    pub kind: AssetKind,
    /// The identity from the file's sidecar, when it has one.
    pub id: Option<Guid>,
}

/// The entries of a directory, folders first and each group in name order.
fn sorted_children(directory: &Path) -> Vec<std::path::PathBuf> {
    let mut children: Vec<_> = fs::read_dir(directory)
        .map(|reader| {
            reader
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect()
        })
        .unwrap_or_default();
    children.sort_by_key(|path| (!path.is_dir(), path.clone()));
    children
}

/// Whether a path is shown: sidecars and hidden entries are not.
fn is_listed(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        !name.starts_with('.') && !name.ends_with(".meta")
    })
}

/// Appends a directory's entries and, below each folder, its own.
fn append_entries(
    root: &Path,
    directory: &Path,
    depth: usize,
    index: &AssetIndex,
    out: &mut Vec<ProjectEntry>,
) {
    for path in sorted_children(directory) {
        if !is_listed(&path) {
            continue;
        }
        let relative = relative_text(root, &path);
        let is_directory = path.is_dir();
        let id = (!is_directory && meta_path(&path).is_file())
            .then(|| index.find_path(&relative).map(|asset| asset.id))
            .flatten();
        out.push(ProjectEntry {
            kind: if is_directory {
                AssetKind::Other
            } else {
                AssetKind::of_path(&relative)
            },
            path: relative,
            depth,
            is_directory,
            id,
        });
        if is_directory {
            append_entries(root, &path, depth + 1, index, out);
        }
    }
}

/// Lists the asset root of a project in tree order.
pub fn list_project(project: &Project) -> Vec<ProjectEntry> {
    let root = project.asset_root();
    let index = AssetIndex::build(&root);
    let mut entries = Vec::new();
    append_entries(&root, &root, 0, &index, &mut entries);
    entries
}
