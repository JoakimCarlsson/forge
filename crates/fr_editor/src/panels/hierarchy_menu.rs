//! The Hierarchy's context menu: Create, Duplicate, Delete and Rename.

use fr_document::Guid;
use fr_ui::{MenuItem, menu_entry, menu_separator, menu_submenu};

use super::hierarchy::HierarchyMessage;
use crate::app::{Action, EditorState, Message};

/// The label of the Create submenu, which is also its expansion key.
pub const CREATE_LABEL: &str = "Create";

/// A message of the Hierarchy as a menu action.
fn hierarchy(message: HierarchyMessage) -> Option<Message> {
    Some(Message::Hierarchy(message))
}

/// A global action as a menu action.
fn action(action: Action) -> Option<Message> {
    Some(Message::Action(action))
}

/// The entries of the Create submenu: one per subsystem entry, then Create
/// Empty, all under `parent`.
fn create_items(state: &EditorState, parent: Guid) -> Vec<MenuItem<Message>> {
    let mut items: Vec<MenuItem<Message>> = state
        .subsystems
        .create_entries()
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            menu_entry(
                entry.label,
                hierarchy(HierarchyMessage::CreateEntry { index, parent }),
            )
            .icon(entry.icon)
        })
        .collect();
    items.push(menu_separator());
    items.push(menu_entry(
        "Create Empty",
        hierarchy(HierarchyMessage::CreateEmpty { parent }),
    ));
    items
}

/// The items of the menu opened on `node`, or on the empty area when none.
pub fn items(state: &EditorState, node: Option<Guid>) -> Vec<MenuItem<Message>> {
    let expanded = state.menu.submenu.as_deref() == Some(CREATE_LABEL);
    let toggle = Some(Message::ToggleSubmenu(CREATE_LABEL.to_owned()));
    let Some(node) = node else {
        return vec![menu_submenu(
            CREATE_LABEL,
            toggle,
            expanded,
            create_items(state, Guid::NONE),
        )];
    };
    let sibling_parent = state.document().map_or(Guid::NONE, |open| {
        open.document().content().node_parent(node)
    });
    let is_entity = state
        .document()
        .is_some_and(|open| open.document().content().find_entity(node).is_some());
    vec![
        menu_submenu(
            CREATE_LABEL,
            toggle,
            expanded,
            create_items(state, sibling_parent),
        ),
        menu_entry(
            "Create Child",
            is_entity.then_some(Message::Hierarchy(HierarchyMessage::CreateEmpty {
                parent: node,
            })),
        ),
        menu_separator(),
        menu_entry("Duplicate", action(Action::Duplicate)).shortcut("Ctrl+D"),
        menu_entry("Delete", action(Action::Delete)).shortcut("Del"),
        menu_entry("Rename", action(Action::Rename)).shortcut("F2"),
    ]
}
