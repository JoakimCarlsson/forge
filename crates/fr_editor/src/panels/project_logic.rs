//! What the Project panel does that needs no interface: the paths of entries,
//! the visible rows of the folder tree and file list, and creating files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use fr_authoring::{SceneTemplate, create_folder, create_prefab_file, create_scene_file};
use fr_document::AssetKind;
use fr_project::ProjectEntry;
use fr_ui::IconName;

/// The kind of entry the panel creates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateKind {
    /// A folder.
    Folder,
    /// A scene with a camera and a sun.
    Scene,
    /// A prefab with one root entity.
    Prefab,
}

impl CreateKind {
    /// The name shown for the kind.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Scene => "Scene",
            Self::Prefab => "Prefab",
        }
    }

    /// The name a new entry of the kind starts with in the prompt.
    pub const fn default_name(self) -> &'static str {
        match self {
            Self::Folder => "New Folder",
            Self::Scene => "New Scene",
            Self::Prefab => "New Prefab",
        }
    }

    /// The icon of the kind's menu entry.
    pub const fn icon(self) -> IconName {
        match self {
            Self::Folder => IconName::FolderAdd,
            Self::Scene | Self::Prefab => IconName::FileAdd,
        }
    }

    /// Every kind in menu order.
    pub const ALL: [Self; 3] = [Self::Folder, Self::Scene, Self::Prefab];
}

/// The folder a path relative to the asset root is in; empty at the root.
pub fn parent_of(relative: &str) -> &str {
    relative.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The last segment of a path relative to the asset root.
pub fn name_of(relative: &str) -> &str {
    relative.rsplit('/').next().unwrap_or(relative)
}

/// The absolute path of a path relative to the asset root; the root for an empty
/// one.
pub fn absolute_of(root: &Path, relative: &str) -> PathBuf {
    if relative.is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    }
}

/// Whether a name contains the query, ignoring case; an empty query matches.
pub fn matches_query(name: &str, query: &str) -> bool {
    let query = query.trim();
    query.is_empty() || name.to_lowercase().contains(&query.to_lowercase())
}

/// One folder of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderRow {
    /// The path relative to the asset root.
    pub path: String,
    /// How many levels below the root row.
    pub depth: usize,
    /// Whether the folder shows its children.
    pub expanded: bool,
    /// Whether the folder has folders in it.
    pub has_folders: bool,
}

/// The folders to draw, in tree order: a folder is drawn when every folder
/// above it is expanded.
pub fn folder_rows(entries: &[ProjectEntry], expanded: &HashSet<String>) -> Vec<FolderRow> {
    let with_folders: HashSet<&str> = entries
        .iter()
        .filter(|entry| entry.is_directory)
        .map(|entry| parent_of(&entry.path))
        .collect();
    let mut open: HashSet<&str> = HashSet::new();
    let mut rows = Vec::new();
    for entry in entries.iter().filter(|entry| entry.is_directory) {
        let parent = parent_of(&entry.path);
        if !parent.is_empty() && !open.contains(parent) {
            continue;
        }
        let is_expanded = expanded.contains(&entry.path);
        if is_expanded {
            open.insert(&entry.path);
        }
        rows.push(FolderRow {
            path: entry.path.clone(),
            depth: entry.depth + 1,
            expanded: is_expanded,
            has_folders: with_folders.contains(entry.path.as_str()),
        });
    }
    rows
}

/// The entries of the file list: the folders then the files of `folder`, or,
/// with a query, every entry whose name matches it.
pub fn list_entries<'a>(
    entries: &'a [ProjectEntry],
    folder: &str,
    query: &str,
) -> Vec<&'a ProjectEntry> {
    let searching = !query.trim().is_empty();
    let mut listed: Vec<&ProjectEntry> = entries
        .iter()
        .filter(|entry| {
            if searching {
                matches_query(name_of(&entry.path), query)
            } else {
                parent_of(&entry.path) == folder
            }
        })
        .collect();
    listed.sort_by_key(|entry| !entry.is_directory);
    listed
}

/// The icon of an entry.
pub fn entry_icon(entry: &ProjectEntry) -> IconName {
    if entry.is_directory {
        IconName::Folder
    } else {
        super::asset_slot::kind_icon(entry.kind)
    }
}

/// Whether double-clicking a file of this kind opens it as a document.
pub fn opens_as_document(kind: AssetKind) -> bool {
    matches!(kind, AssetKind::Scene | AssetKind::Prefab)
}

/// Creates a folder, scene or prefab named `name` in `folder` of the asset root
/// and returns its absolute path. A scene and a prefab get their `.meta`
/// sidecar.
///
/// # Errors
///
/// The message of the refusal: an unacceptable or taken name, a folder outside
/// the asset root, or the file system.
pub fn create_entry(
    asset_root: &Path,
    folder: &str,
    kind: CreateKind,
    name: &str,
) -> Result<PathBuf, String> {
    let directory = absolute_of(asset_root, folder);
    match kind {
        CreateKind::Folder => create_folder(asset_root, &directory, name),
        CreateKind::Scene => {
            create_scene_file(asset_root, &directory, name, SceneTemplate::Starter)
                .map(|created| created.path)
        }
        CreateKind::Prefab => {
            create_prefab_file(asset_root, &directory, name).map(|created| created.path)
        }
    }
    .map_err(|error| error.to_string())
}
