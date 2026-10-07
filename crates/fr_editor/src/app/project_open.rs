//! Opening, creating and closing a project.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fr_authoring::Workspace;
use fr_document::builtin_schemas;
use fr_project::{Project, create_project, engine_crate_dir};

use super::documents::open_path;
use super::state::EditorState;
use crate::panels::hierarchy_state::HierarchyState;
use crate::panels::inspector::InspectorPanelState;
use crate::panels::project::ProjectPanelState;
use crate::preview::Preview;
use crate::settings::ProjectState;
use crate::viewport::Viewport;

/// Writes the open documents of the project to its `.forge-editor` folder.
pub fn save_project_state(state: &mut EditorState) {
    let Some(project) = state.project.as_ref().filter(|_| state.persist) else {
        return;
    };
    let root = state.workspace.asset_root();
    let relative = |path: &Path| {
        path.strip_prefix(root).map_or_else(
            |_| path.display().to_string(),
            |rest| rest.display().to_string(),
        )
    };
    let open_documents = state
        .workspace
        .documents()
        .map(|(_, open)| relative(open.document().path()))
        .collect();
    let current = state
        .document()
        .map(|open| relative(open.document().path()));
    let saved = ProjectState {
        open_documents,
        current,
    };
    if let Err(error) = saved.save(project.root()) {
        state.error(format!("cannot keep the editor state: {error}"));
    }
}

/// Restores the documents a project had open, or opens its startup scene.
fn restore_documents(state: &mut EditorState, project_root: &Path, startup: Option<String>) {
    let remembered = ProjectState::load(project_root).unwrap_or_else(|error| {
        state.error(format!("cannot read the editor state: {error}"));
        ProjectState::default()
    });
    let root = state.workspace.asset_root().to_path_buf();
    for relative in &remembered.open_documents {
        let path = root.join(relative);
        if path.is_file() {
            open_path(state, &path);
        }
    }
    if state.workspace.is_empty()
        && let Some(startup) = startup
    {
        open_path(state, &root.join(startup));
    }
    if let Some(current) = remembered.current
        && let Some(id) = state.workspace.find_by_path(&root.join(current))
    {
        let _ = state.workspace.set_current(id);
    }
}

/// Opens a project: reads it, restores its documents and remembers it.
///
/// # Errors
///
/// The message of the project that could not be opened.
pub fn open_project(state: &mut EditorState, path: &Path) -> Result<(), String> {
    let project = Project::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    save_project_state(state);
    let asset_root = project.asset_root();
    let startup = project.startup_scene().map(str::to_owned);
    let name = project.settings().name.clone();
    let root = project.root().to_path_buf();
    state.workspace = Workspace::new(asset_root.clone(), Arc::new(builtin_schemas()));
    state.preview = Preview::new(&asset_root);
    state.viewport = Viewport::new();
    state.hierarchy = HierarchyState::default();
    state.project_panel = ProjectPanelState::default();
    state.inspector_panel = InspectorPanelState::default();
    state.text.target = None;
    state.popup = None;
    state.picker = None;
    state.project = Some(project);
    state.settings.remember_project(&root);
    state.persist_settings();
    state.info(format!("opened project {name} ({})", root.display()));
    restore_documents(state, &root, startup);
    Ok(())
}

/// Closes the project, leaving the picker.
pub fn close_project(state: &mut EditorState) {
    save_project_state(state);
    state.project = None;
    state.workspace = Workspace::new(PathBuf::new(), Arc::new(builtin_schemas()));
    state.preview = Preview::new(Path::new(""));
    state.hierarchy = HierarchyState::default();
    state.text.target = None;
    state.popup = None;
    state.show_picker();
}

/// Creates a project named `name` in the projects folder and opens it.
///
/// # Errors
///
/// The message of the project that could not be created or opened.
pub fn new_project(state: &mut EditorState, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err("give the project a name without slashes".to_owned());
    }
    let root = state.settings.projects_root().join(name);
    create_project(&root, &engine_crate_dir())
        .map_err(|error| format!("cannot create {}: {error}", root.display()))?;
    open_project(state, &root)
}
