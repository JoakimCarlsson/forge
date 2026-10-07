//! The Inspector panel: the properties of the selection.
//!
//! The panel shows the primary selected node: its name and activity, its
//! transform and one collapsible section per component, or the scene settings
//! when nothing is selected in a scene. Every row is an
//! [`InspectorRow`](crate::subsystems::InspectorRow) the subsystem registry
//! supplies, drawn by [`rows`](super::rows), and every edit is an
//! `fr_authoring` command on the current document: a drag or a colour slider is
//! one gesture and so one undo entry, anything else is one command.
//!
//! Text goes through the app's text focus under [`TextTarget::Inspector`]
//! names: the app asks [`text_source`] what a field starts with and calls
//! [`text_ended`] when a field stops being typed into.
//!
//! Modules:
//!
//! - [`inspector_rows`](super::inspector_rows), [`inspector_scene`](super::inspector_scene):
//!   the rows of a node, its components and the scene
//! - [`inspector_view`](super::inspector_view): the layout
//! - [`row_editor`](super::row_editor), [`rows`](super::rows),
//!   [`asset_slot`](super::asset_slot): the editors the rows use

use std::collections::HashSet;

use fr_authoring::{Command, plan_add_component};
use fr_document::Guid;
use fr_ui::{Div, Scroll, ScrollEvent, Theme};

use super::inspector_rows::rows_of;
use super::inspector_view;
use super::row_editor::{
    Effect, RowEditor, RowEvent, RowKey, TextRole, TextSite, hex_value, text_value,
};
use super::text_end::TextEnd;
use crate::app::{EditorState, Message, TextTarget};
use crate::subsystems::InspectorRow;

/// The name of the add-component menu's search field.
const ADD_SEARCH: &str = "add-search";

/// A menu the panel can have open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectorMenu {
    /// The list of components that can be added.
    AddComponent,
    /// The menu of one component's header.
    Section(Guid),
}

/// What the user did in the Inspector panel.
#[derive(Clone, Debug, PartialEq)]
pub enum InspectorMessage {
    /// Something happened to a row's widgets.
    Row(RowKey, RowEvent),
    /// The panel was scrolled.
    Scroll(ScrollEvent),
    /// A section was collapsed or expanded.
    ToggleSection(Guid),
    /// A component was enabled or disabled.
    SetEnabled {
        /// The component.
        component: Guid,
        /// Whether it takes part in the scene.
        enabled: bool,
    },
    /// A component was removed.
    Remove(Guid),
    /// A menu was opened, or closed when it was the open one.
    ToggleMenu(InspectorMenu),
    /// A press landed outside the open menu.
    CloseMenu,
    /// The open menu's list was scrolled.
    MenuScroll(ScrollEvent),
    /// The pointer went down or dragged in the add-component search, at this
    /// character.
    SearchCaret(usize, bool),
    /// A component type was chosen in the add-component menu.
    AddComponent(String),
}

/// The Inspector panel's own state, kept in the [`EditorState`].
#[derive(Debug, Default)]
pub struct InspectorPanelState {
    /// Gestures, popups and scrolling of the rows.
    pub(super) editor: RowEditor,
    /// How far the panel is scrolled.
    pub(super) scroll: Scroll,
    /// The sections that are collapsed, by component or node identity.
    pub(super) collapsed: HashSet<Guid>,
    /// The menu that is open.
    pub(super) menu: Option<InspectorMenu>,
    /// How far the open menu's list is scrolled.
    pub(super) menu_scroll: Scroll,
    /// The search typed into the open picker or menu, kept while the search
    /// does not have the keyboard.
    pub(super) search: String,
}

/// The panel's contents.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    inspector_view::view(state, theme)
}

/// The node the Inspector shows: the primary selected node of the current
/// document.
fn primary(state: &EditorState) -> Option<Guid> {
    state.document()?.selection().primary()
}

/// The row a key names in the current document as it is now.
fn row_for(state: &EditorState, key: &RowKey) -> Option<InspectorRow> {
    let document = state.document()?;
    let lock_name = document.selection().len() > 1;
    rows_of(
        &state.subsystems,
        document,
        primary(state),
        key.owner,
        lock_name,
    )
    .into_iter()
    .find(|row| row.key == key.key)
}

/// The field of this panel that has the keyboard, when one does.
fn typing_site(state: &EditorState) -> Option<TextSite> {
    state.text.target.as_ref().and_then(TextSite::from_target)
}

/// The text a field of this panel starts with when it is given the keyboard.
pub fn text_source(state: &EditorState, target: &TextTarget) -> String {
    let searching = *target == add_search_target()
        || TextSite::from_target(target).is_some_and(|site| site.role == TextRole::Search);
    if searching {
        state.inspector_panel.search.clone()
    } else {
        String::new()
    }
}

/// Applies a message of the panel.
pub fn update(state: &mut EditorState, message: InspectorMessage) {
    match message {
        InspectorMessage::Row(key, event) => row_event(state, key, event),
        InspectorMessage::Scroll(event) => state.inspector_panel.scroll.apply(event),
        InspectorMessage::ToggleSection(id) => toggle_section(state, id),
        InspectorMessage::SetEnabled { component, enabled } => {
            set_enabled(state, component, enabled);
        }
        InspectorMessage::Remove(component) => remove_component(state, component),
        InspectorMessage::ToggleMenu(menu) => toggle_menu(state, menu),
        InspectorMessage::CloseMenu => close_menu(state),
        InspectorMessage::MenuScroll(event) => state.inspector_panel.menu_scroll.apply(event),
        InspectorMessage::SearchCaret(index, extend) => {
            focus_add_search(state);
            state.text.edit.set_caret(index, extend);
        }
        InspectorMessage::AddComponent(kind) => add_component(state, &kind),
    }
}

/// Forgets what is open and typed, abandoning a gesture, as when the document
/// or the selection changes.
pub fn reset(state: &mut EditorState) {
    let document = state.workspace.current_mut();
    if let Err(error) = state.inspector_panel.editor.reset(document) {
        state.error(error.to_string());
    }
    state.inspector_panel.menu = None;
    if matches!(state.text.target, Some(TextTarget::Inspector(_))) {
        state.text.end();
    }
}

/// Closes the popup or menu the panel has open, as Escape does; false when
/// nothing was open.
pub fn dismiss(state: &mut EditorState) -> bool {
    let panel = &state.inspector_panel;
    let open = panel.menu.is_some() || panel.editor.popup().is_some();
    if open {
        close_menu(state);
        if let Some(popup) = state.inspector_panel.editor.popup().cloned() {
            row_event(state, popup.row, RowEvent::DismissPopup);
        }
    }
    open
}

/// The text focus name of the add-component search.
fn add_search_target() -> TextTarget {
    TextTarget::Inspector(ADD_SEARCH.to_owned())
}

/// Collapses a section, or expands it.
fn toggle_section(state: &mut EditorState, id: Guid) {
    let collapsed = &mut state.inspector_panel.collapsed;
    if !collapsed.remove(&id) {
        collapsed.insert(id);
    }
}

/// Runs a command on the current document, reporting a failure.
fn run(state: &mut EditorState, command: &Command) -> bool {
    crate::app::edit::run(state, command).is_some()
}

/// Enables or disables a component of the primary entity.
fn set_enabled(state: &mut EditorState, component: Guid, enabled: bool) {
    let Some(entity) = primary(state) else {
        return;
    };
    run(
        state,
        &Command::SetComponentEnabled {
            entity,
            component,
            enabled,
        },
    );
}

/// Removes a component of the primary entity, when the rules allow it.
fn remove_component(state: &mut EditorState, component: Guid) {
    let Some(entity) = primary(state) else {
        return;
    };
    close_menu(state);
    run(state, &Command::RemoveComponent { entity, component });
}

/// Gives the keyboard to the add-component search when it does not have it.
fn focus_add_search(state: &mut EditorState) {
    let target = add_search_target();
    if !state.text.has(&target) {
        let search = state.inspector_panel.search.clone();
        state.text.begin(target, &search);
    }
}

/// Gives the keyboard to a row's picker search when it does not have it.
fn focus_row_search(state: &mut EditorState, key: &RowKey) {
    let site = TextSite {
        row: key.clone(),
        role: TextRole::Search,
    };
    if !state.text.has(&site.target()) {
        let search = state.inspector_panel.search.clone();
        state.text.begin(site.target(), &search);
    }
}

/// Applies an event of a row's widgets.
fn row_event(state: &mut EditorState, key: RowKey, event: RowEvent) {
    if matches!(event, RowEvent::SearchCaret(..)) {
        focus_row_search(state, &key);
    }
    if matches!(event, RowEvent::TogglePopup) {
        state.inspector_panel.search.clear();
    }
    flush_text(state, &event);
    let Some(row) = row_for(state, &key) else {
        return;
    };
    if row.read_only {
        return;
    }
    let panel = &mut state.inspector_panel;
    let Some(document) = state.workspace.current_mut() else {
        return;
    };
    match panel.editor.event(document, &row, key, event) {
        Ok(effect) => apply_effect(state, effect),
        Err(error) => state.error(error.to_string()),
    }
}

/// Stores the number or text being typed into a row's slot before another
/// widget takes over, as leaving a field does.
fn flush_text(state: &mut EditorState, event: &RowEvent) {
    if matches!(
        event,
        RowEvent::Caret(..) | RowEvent::HexCaret(..) | RowEvent::SearchCaret(..)
    ) {
        return;
    }
    let Some(site) = typing_site(state) else {
        return;
    };
    if matches!(site.role, TextRole::Slot(_)) {
        let text = state.text.edit.text().to_owned();
        state.text.end();
        store_text(state, &site, &text);
    }
}

/// Stores a typed text through its row as one undo entry; text that is not what
/// the row takes is dropped.
fn store_text(state: &mut EditorState, site: &TextSite, text: &str) {
    let Some(row) = row_for(state, &site.row) else {
        return;
    };
    let value = match site.role {
        TextRole::Slot(slot) => text_value(&row, slot, text),
        TextRole::Hex => hex_value(text),
        TextRole::Search => None,
    };
    if let Some(value) = value {
        row_event(state, site.row.clone(), RowEvent::Change(value));
    }
}

/// Does what an event asked of the text focus.
fn apply_effect(state: &mut EditorState, effect: Effect) {
    match effect {
        Effect::None => {}
        Effect::Begin(site, text) => state.text.begin(site.target(), &text),
        Effect::End => {
            let popup_text = typing_site(state)
                .is_some_and(|site| matches!(site.role, TextRole::Hex | TextRole::Search));
            if popup_text {
                state.text.end();
            }
        }
        Effect::Caret(index, extend) => {
            if typing_site(state).is_some() {
                state.text.edit.set_caret(index, extend);
            }
        }
    }
}

/// A field of this panel stopped being typed into. Enter and a click elsewhere
/// store what a number, text or colour field says; Escape drops it. A search keeps what was typed while it
/// does not have the keyboard, and Escape closes its popup or menu.
pub fn text_ended(state: &mut EditorState, target: &TextTarget, text: &str, end: TextEnd) {
    let commit = end.commits();
    if *target == add_search_target() {
        if commit {
            state.inspector_panel.search = text.to_owned();
        } else {
            close_menu(state);
        }
        return;
    }
    let Some(site) = TextSite::from_target(target) else {
        return;
    };
    match (site.role, commit) {
        (TextRole::Slot(_) | TextRole::Hex, true) => store_text(state, &site, text),
        (TextRole::Search, true) => state.inspector_panel.search = text.to_owned(),
        (TextRole::Search, false) => row_event(state, site.row, RowEvent::DismissPopup),
        _ => {}
    }
}

/// Opens a menu, or closes it when it is the open one.
fn toggle_menu(state: &mut EditorState, menu: InspectorMenu) {
    if state.inspector_panel.menu == Some(menu) {
        close_menu(state);
        return;
    }
    state.inspector_panel.menu = Some(menu);
    state.inspector_panel.menu_scroll.scroll_to(0.0);
    state.inspector_panel.search.clear();
    if menu == InspectorMenu::AddComponent {
        state.text.begin(add_search_target(), "");
    }
}

/// Closes the open menu and gives the keyboard back.
fn close_menu(state: &mut EditorState) {
    state.inspector_panel.menu = None;
    if state.text.has(&add_search_target()) {
        state.text.end();
    }
}

/// Adds a component type to the primary entity, with the components it needs as
/// one undo entry, and says which others came with it.
fn add_component(state: &mut EditorState, kind: &str) {
    let Some(entity_id) = primary(state) else {
        return;
    };
    let Some(document) = state.document() else {
        return;
    };
    let schemas = document.document().schemas();
    let Some(entity) = document.document().content().find_entity(entity_id) else {
        return;
    };
    let extras: Vec<String> = plan_add_component(entity, kind, schemas)
        .map(|plan| {
            plan.prerequisites()
                .iter()
                .map(|key| {
                    schemas
                        .find(key)
                        .map_or(key.clone(), |schema| schema.label.clone())
                })
                .collect()
        })
        .unwrap_or_default();
    close_menu(state);
    let added = run(
        state,
        &Command::AddComponent {
            entity: entity_id,
            kind: kind.to_owned(),
        },
    );
    if added && !extras.is_empty() {
        state.info(format!("Also added: {}", extras.join(", ")));
    }
}
