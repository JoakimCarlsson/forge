//! What the Hierarchy's messages do: select, rename, reparent, create and the
//! rest, all as commands on the current document.

use fr_authoring::{Command, CreateEntity};
use fr_document::Guid;
use fr_math::Vec3;
use fr_ui::{DragPhase, DropHint, Point, drop_zone};

use super::hierarchy::HierarchyMessage;
use super::hierarchy_state::{DropTarget, HierarchyDrag};
use crate::app::{EditorState, Popup, PopupKind, edit};

/// The prefix of the keys the rows are recorded under.
pub const ROW_PREFIX: &str = "h:";

/// The node a row key stands for; unset for the document's own row.
fn node_of_key(key: &str) -> Option<Guid> {
    let rest = key.strip_prefix(ROW_PREFIX)?;
    if rest == "root" {
        return Some(Guid::NONE);
    }
    Guid::from_text(rest)
}

/// The row under a point and the part of it the point is over.
fn target_at(state: &EditorState, position: Point) -> Option<DropTarget> {
    let key = state.rects.entries().into_iter().find_map(|(key, rect)| {
        (key.starts_with(ROW_PREFIX) && rect.contains(position)).then_some((key, rect))
    })?;
    let node = node_of_key(&key.0)?;
    let hint = if node.valid() {
        drop_zone(key.1, position.y)
    } else {
        DropHint::Onto
    };
    Some(DropTarget { node, hint })
}

/// Whether a carried node may land on a target: not on itself or below itself.
fn may_land(state: &EditorState, node: Guid, target: DropTarget) -> bool {
    let Some(open) = state.document() else {
        return false;
    };
    let content = open.document().content();
    target.node != node && !fr_authoring::subtree_ids(content, node).contains(&target.node)
}

/// The command a drop makes: an inside drop appends, a before or after drop
/// places the node beside the target.
fn drop_command(state: &EditorState, node: Guid, target: DropTarget) -> Option<Command> {
    let content = state.document()?.document().content();
    if target.hint == DropHint::Onto || !target.node.valid() {
        return Some(Command::Reparent {
            node,
            parent: target.node,
            index: None,
        });
    }
    let parent = content.node_parent(target.node);
    let siblings: Vec<Guid> = content
        .child_nodes(parent)
        .into_iter()
        .map(|child| child.id)
        .filter(|id| *id != node)
        .collect();
    let position = siblings.iter().position(|id| *id == target.node)?;
    let index = position + usize::from(target.hint == DropHint::After);
    Some(Command::Reparent {
        node,
        parent,
        index: Some(index),
    })
}

/// Follows a carried row, and performs the drop when it is let go.
fn drag(state: &mut EditorState, node: Guid, event: fr_ui::DragEvent) {
    match event.phase {
        DragPhase::Began | DragPhase::Moved => {
            let target =
                target_at(state, event.position).filter(|target| may_land(state, node, *target));
            state.hierarchy.drag = Some(HierarchyDrag { node, target });
        }
        DragPhase::Cancelled => state.hierarchy.drag = None,
        DragPhase::Ended => {
            let target = state.hierarchy.drag.take().and_then(|drag| drag.target);
            let Some(command) = target.and_then(|target| drop_command(state, node, target)) else {
                return;
            };
            if let Some(parent) = match &command {
                Command::Reparent { parent, .. } => Some(*parent),
                _ => None,
            } && let Some(id) = state.document_id()
            {
                state.hierarchy.expand(id, parent);
            }
            edit::run(state, &command);
        }
    }
}

/// Selects a node the way a click with the held modifiers does.
fn select(state: &mut EditorState, node: Guid) {
    let modifiers = state.modifiers;
    let Some(open) = state.document_mut() else {
        return;
    };
    if modifiers.control || modifiers.logo {
        open.selection_mut().toggle(node);
    } else if modifiers.shift {
        let content = open.document().content().clone();
        open.selection_mut().extend_to(&content, node);
    } else {
        open.selection_mut().set(node);
    }
    state.focused_panel = Some(crate::panels::registry::ids::HIERARCHY.to_owned());
}

/// Creates the entity a Create menu entry describes under `parent` and selects
/// it.
fn create_from_entry(state: &mut EditorState, index: usize, parent: Guid) {
    let Some(entry) = state.subsystems.create_entries().into_iter().nth(index) else {
        return;
    };
    let command = entry.command(parent, Vec3::ZERO);
    created(state, &command, parent);
}

/// Runs a creating command, selects what it made and expands its parent.
fn created(state: &mut EditorState, command: &Command, parent: Guid) {
    if let Some(outcome) = edit::run(state, command) {
        edit::select(state, &outcome.created);
        if let Some(id) = state.document_id() {
            state.hierarchy.expand(id, parent);
        }
    }
}

/// Applies a message of the Hierarchy.
pub fn update(state: &mut EditorState, message: HierarchyMessage) {
    match message {
        HierarchyMessage::Select(node) => select(state, node),
        HierarchyMessage::ClearSelection => {
            if let Some(open) = state.document_mut() {
                open.selection_mut().clear();
            }
        }
        HierarchyMessage::ToggleExpand(node) => {
            if let Some(id) = state.document_id() {
                state.hierarchy.toggle(id, node);
            }
        }
        HierarchyMessage::BeginRename(node) => {
            if let Some(open) = state.document_mut() {
                open.selection_mut().set(node);
            }
            edit::begin_rename(state);
        }
        HierarchyMessage::Drag { node, event } => drag(state, node, event),
        HierarchyMessage::Context(node) => {
            if let Some(node) = node.filter(|node| {
                state
                    .document()
                    .is_some_and(|open| !open.selection().contains(*node))
            }) && let Some(open) = state.document_mut()
            {
                open.selection_mut().set(node);
            }
            state.menu.submenu = None;
            state.menu.open = None;
            state.popup = Some(Popup {
                origin: state.pointer,
                kind: PopupKind::Hierarchy(node),
            });
        }
        HierarchyMessage::CreateEntry { index, parent } => {
            state.popup = None;
            create_from_entry(state, index, parent);
        }
        HierarchyMessage::CreateEmpty { parent } => {
            state.popup = None;
            let command = Command::CreateEntity(CreateEntity::new("Entity").under(parent));
            created(state, &command, parent);
        }
        HierarchyMessage::Scroll(event) => state.hierarchy.scroll.apply(event),
    }
}

/// Applies a renamed node's new name when the field is left with Enter or a
/// click elsewhere; an empty name leaves the node as it was.
pub fn commit_rename(state: &mut EditorState, node: Guid, name: &str) {
    state.hierarchy.renaming = None;
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    let unchanged = state.document().is_some_and(|open| {
        open.document()
            .content()
            .find_entity(node)
            .is_some_and(|entity| entity.name == name)
            || open
                .document()
                .content()
                .find_instance(node)
                .is_some_and(|instance| instance.name == name)
    });
    if unchanged {
        return;
    }
    edit::run(
        state,
        &Command::Rename {
            node,
            name: name.to_owned(),
        },
    );
}

/// Leaves a rename without applying it.
pub fn cancel_rename(state: &mut EditorState) {
    state.hierarchy.renaming = None;
}
