//! The menu bar and the items of each menu, with their disabled states read
//! from the state.

use fr_ui::{
    Div, MenuItem, Theme, menu_bar, menu_bar_entry, menu_entry, menu_separator, menu_submenu,
};

use super::message::{Action, MenuKind, Message};
use super::state::EditorState;
use crate::panels::registry::PANELS;
use crate::run::RunState;

/// The label of the Open Recent submenu, which is also its expansion key.
const RECENT_LABEL: &str = "Open Recent";

/// An entry sending `action`, greyed out unless `enabled`.
fn entry(label: &str, action: Action, enabled: bool) -> MenuItem<Message> {
    menu_entry(label, enabled.then_some(Message::Action(action)))
}

/// The File menu.
fn file_items(state: &EditorState) -> Vec<MenuItem<Message>> {
    let project = state.has_project();
    let current = state.document().is_some();
    let dirty = !state.workspace.dirty_documents().is_empty();
    let recent: Vec<MenuItem<Message>> = state
        .settings
        .recent_projects()
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            menu_entry(name, Some(Message::Action(Action::OpenRecent(index))))
        })
        .collect();
    let recent_enabled = !recent.is_empty();
    vec![
        entry("New Scene", Action::NewScene, project).shortcut("Ctrl+N"),
        entry("New Prefab", Action::NewPrefab, project).shortcut("Ctrl+Shift+N"),
        menu_separator(),
        entry("Open Project...", Action::OpenProject, true).shortcut("Ctrl+O"),
        menu_submenu(
            RECENT_LABEL,
            recent_enabled.then(|| Message::ToggleSubmenu(RECENT_LABEL.to_owned())),
            state.menu.submenu.as_deref() == Some(RECENT_LABEL),
            recent,
        ),
        menu_separator(),
        entry("Save", Action::Save, current).shortcut("Ctrl+S"),
        entry("Save All", Action::SaveAll, dirty).shortcut("Ctrl+Shift+S"),
        menu_separator(),
        entry("Close Project", Action::CloseProject, project),
        entry("Quit", Action::Quit, true).shortcut("Ctrl+Q"),
    ]
}

/// The Edit menu, its Undo and Redo named after the edit they act on.
fn edit_items(state: &EditorState) -> Vec<MenuItem<Message>> {
    let history = state.document().map(|open| open.history());
    let undo_label = history
        .and_then(|history| history.undo_label())
        .map_or_else(|| "Undo".to_owned(), |label| format!("Undo {label}"));
    let redo_label = history
        .and_then(|history| history.redo_label())
        .map_or_else(|| "Redo".to_owned(), |label| format!("Redo {label}"));
    let can_undo = history.is_some_and(|history| history.can_undo());
    let can_redo = history.is_some_and(|history| history.can_redo());
    let selected = state
        .document()
        .is_some_and(|open| !open.selection().is_empty());
    vec![
        entry(&undo_label, Action::Undo, can_undo).shortcut("Ctrl+Z"),
        entry(&redo_label, Action::Redo, can_redo).shortcut("Ctrl+Shift+Z"),
        menu_separator(),
        entry("Delete", Action::Delete, selected).shortcut("Del"),
        entry("Duplicate", Action::Duplicate, selected).shortcut("Ctrl+D"),
        entry("Rename", Action::Rename, selected).shortcut("F2"),
    ]
}

/// The Project menu.
fn project_items(state: &EditorState) -> Vec<MenuItem<Message>> {
    let project = state.has_project();
    let idle = state.runner.state() == RunState::Idle;
    vec![
        entry("Build", Action::Build, project && idle),
        menu_separator(),
        entry("Reveal Asset Root", Action::RevealAssetRoot, project),
        entry("Project Settings", Action::ShowProjectInfo, project),
    ]
}

/// The Play menu: Stop replaces the play entries while something runs.
fn play_items(state: &EditorState) -> Vec<MenuItem<Message>> {
    let project = state.has_project();
    if state.runner.state() == RunState::Idle {
        vec![
            entry(
                "Save and Play",
                Action::Play { build_first: false },
                project,
            )
            .shortcut("F5"),
            entry(
                "Build and Play",
                Action::Play { build_first: true },
                project,
            ),
        ]
    } else {
        vec![entry("Stop", Action::Stop, true).shortcut("Shift+F5")]
    }
}

/// The View menu: a switch per panel, Frame Selection and Reset Layout.
fn view_items(state: &EditorState) -> Vec<MenuItem<Message>> {
    let mut items: Vec<MenuItem<Message>> = PANELS
        .iter()
        .map(|panel| {
            entry(panel.title, Action::TogglePanel(panel.id), true)
                .checked(state.dock.contains(panel.id))
        })
        .collect();
    items.push(menu_separator());
    items.push(
        entry(
            "Frame Selection",
            Action::FrameSelection,
            state.document().is_some(),
        )
        .shortcut("F"),
    );
    items.push(entry("Reset Layout", Action::ResetLayout, true));
    items
}

/// The menu bar across the top of the window.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    let titled: [(MenuKind, &str, Vec<MenuItem<Message>>); 5] = [
        (MenuKind::File, "File", file_items(state)),
        (MenuKind::Edit, "Edit", edit_items(state)),
        (MenuKind::Project, "Project", project_items(state)),
        (MenuKind::Play, "Play", play_items(state)),
        (MenuKind::View, "View", view_items(state)),
    ];
    let entries = titled
        .into_iter()
        .map(|(kind, title, items)| {
            menu_bar_entry(
                title,
                state.menu.open == Some(kind),
                Message::ToggleMenu(kind),
                Message::CloseMenus,
                items,
            )
        })
        .collect();
    menu_bar(theme, entries)
}
