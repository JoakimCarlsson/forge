//! Opening, creating, saving and closing documents.

use std::path::{Path, PathBuf};

use fr_authoring::{DocumentId, DocumentKind};

use super::project_open::save_project_state;
use super::prompt::{Pending, Prompt};
use super::state::EditorState;
use crate::panels::console::ConsoleLevel;

/// The folder new scenes and prefabs go in: `scenes` or `prefabs` when the
/// project has one, else the asset root.
fn folder_for(state: &EditorState, kind: DocumentKind) -> PathBuf {
    let root = state.workspace.asset_root().to_path_buf();
    let preferred = match kind {
        DocumentKind::Scene => root.join("scenes"),
        DocumentKind::Prefab => root.join("prefabs"),
    };
    if preferred.is_dir() { preferred } else { root }
}

/// A path for a new `untitled` document that nothing uses yet.
fn unused_path(state: &EditorState, kind: DocumentKind) -> PathBuf {
    let folder = folder_for(state, kind);
    (1..)
        .map(|number| {
            let stem = if number == 1 {
                "untitled".to_owned()
            } else {
                format!("untitled {number}")
            };
            folder.join(format!("{stem}.{}", kind.extension()))
        })
        .find(|path| !path.exists() && state.workspace.find_by_path(path).is_none())
        .unwrap_or_else(|| folder.join("untitled.scene"))
}

/// Opens a scene or prefab file, or shows it when it is open, and remembers the
/// open documents.
pub fn open_path(state: &mut EditorState, path: &Path) {
    match state.workspace.open(path) {
        Ok(_) => save_project_state(state),
        Err(error) => state.error(format!("cannot open {}: {error}", path.display())),
    }
}

/// Creates an unsaved document of a kind and shows it.
pub fn new_document(state: &mut EditorState, kind: DocumentKind) {
    if !state.has_project() {
        return;
    }
    let path = unused_path(state, kind);
    let created = match kind {
        DocumentKind::Scene => state.workspace.new_scene(&path),
        DocumentKind::Prefab => state.workspace.new_prefab(&path),
    };
    match created {
        Ok(_) => {
            state.info(format!("created {}", path.display()));
            save_project_state(state);
        }
        Err(error) => state.error(format!("cannot create a {}: {error}", kind.name())),
    }
}

/// Saves the current document.
pub fn save_current(state: &mut EditorState) {
    let Some(id) = state.document_id() else {
        return;
    };
    match state.workspace.save(id) {
        Ok(()) => {
            let title = state
                .document()
                .map(|open| open.title())
                .unwrap_or_default();
            state.toast = Some(format!("Saved {title}"));
            state.log(ConsoleLevel::Info, format!("saved {title}"));
        }
        Err(error) => state.error(format!("save failed: {error}")),
    }
}

/// Saves every document with changes, reporting each failure; the first is the
/// error.
///
/// # Errors
///
/// The message of the first document that could not be saved.
pub fn save_all(state: &mut EditorState) -> Result<(), String> {
    let results = state.workspace.save_all();
    let mut first = None;
    for saved in results {
        match saved.result {
            Ok(()) => state.info(format!("saved {}", saved.path.display())),
            Err(error) => {
                let message = format!("cannot save {}: {error}", saved.path.display());
                state.error(message.clone());
                first.get_or_insert(message);
            }
        }
    }
    first.map_or(Ok(()), Err)
}

/// Closes a document, asking first when it has unsaved changes.
pub fn request_close(state: &mut EditorState, id: DocumentId) {
    let dirty = state.workspace.get(id).is_some_and(|open| open.is_dirty());
    if dirty {
        let title = state
            .workspace
            .get(id)
            .map(|open| open.title())
            .unwrap_or_default();
        state.prompt = Some(Prompt {
            text: format!("Save changes to {title}?"),
            then: Pending::CloseDocument(id),
        });
    } else {
        close_now(state, id);
    }
}

/// Closes a document without asking.
pub fn close_now(state: &mut EditorState, id: DocumentId) {
    if state.workspace.close(id).is_ok() {
        state.hierarchy.forget(id);
        state.hierarchy.renaming = None;
        state.text.target = None;
        save_project_state(state);
    }
}

/// The scene document to play: the current one when it is a scene, else any
/// open scene, else the project's startup scene opened for the purpose.
///
/// # Errors
///
/// A message when no scene is open and the project names no startup scene.
pub fn scene_to_play(state: &mut EditorState) -> Result<String, String> {
    let is_scene =
        |open: &fr_authoring::OpenDocument| open.document().kind() == DocumentKind::Scene;
    if let Some(open) = state.document().filter(|open| is_scene(open)) {
        return Ok(open.document().relative_path().to_owned());
    }
    if let Some((_, open)) = state.workspace.documents().find(|(_, open)| is_scene(open)) {
        return Ok(open.document().relative_path().to_owned());
    }
    let startup = state
        .project
        .as_ref()
        .and_then(|project| project.startup_scene().map(str::to_owned))
        .ok_or_else(|| "open a scene to play".to_owned())?;
    let path = state.workspace.asset_root().join(&startup);
    open_path(state, &path);
    state
        .workspace
        .find_by_path(&path)
        .map(|_| startup)
        .ok_or_else(|| "the startup scene could not be opened".to_owned())
}
