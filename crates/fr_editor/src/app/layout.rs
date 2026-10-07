//! The dock layout: loading the saved one, keeping changes and resetting.

use std::fs;

use fr_ui::{DockChange, DockEvent, DockTree};

use super::state::EditorState;
use crate::panels::registry::{self, default_layout, reconcile};

/// Loads `layout.txt` from the config folder into the state; a missing file is
/// the default layout and an unreadable one is reported and replaced by it.
pub fn load_layout(state: &mut EditorState) {
    let path = state.settings.layout_path();
    if !path.is_file() {
        state.dock = default_layout();
        return;
    }
    let loaded = fs::read_to_string(&path)
        .map_err(|error| error.to_string())
        .and_then(|text| DockTree::from_text(&text).map_err(|error| error.to_string()));
    match loaded {
        Ok(layout) => state.dock = reconcile(layout),
        Err(error) => {
            state.error(format!(
                "cannot use the layout in {}: {error}; the default layout is used",
                path.display()
            ));
            state.dock = default_layout();
        }
    }
}

/// Writes the layout when it changed since it was last written.
pub fn save_layout(state: &mut EditorState) {
    if !state.layout_dirty {
        return;
    }
    state.layout_dirty = false;
    if !state.persist {
        return;
    }
    let path = state.settings.layout_path();
    let written = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(&path, state.dock.to_text()));
    if let Err(error) = written {
        state.error(format!("cannot write {}: {error}", path.display()));
    }
}

/// Applies what the user did to the dock.
pub fn apply_dock_event(state: &mut EditorState, event: DockEvent) {
    if let DockEvent::Select { id } = &event {
        state.focused_panel = Some(id.clone());
    }
    let change = state.dock.apply(event, &state.rects, &mut state.dock_drag);
    if change == DockChange::Settled {
        state.layout_dirty = true;
    }
}

/// Shows or hides a panel.
pub fn toggle_panel(state: &mut EditorState, id: &str) {
    registry::toggle(&mut state.dock, id);
    state.layout_dirty = true;
}

/// Puts the panels back where they start.
pub fn reset_layout(state: &mut EditorState) {
    state.dock = default_layout();
    state.layout_dirty = true;
}
