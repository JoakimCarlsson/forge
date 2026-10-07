//! Editing the current document through `fr_authoring` commands: the one place
//! the app runs a command, reports its failure and keeps the selection.

use fr_authoring::{Command, CommandOutcome};
use fr_document::{EntityDocument, Guid};

use super::message::TextTarget;
use super::state::EditorState;

/// Runs a command on the current document as one undo entry, reporting a
/// failure in the console.
pub fn run(state: &mut EditorState, command: &Command) -> Option<CommandOutcome> {
    let Some(open) = state.document_mut() else {
        state.error("no document is open");
        return None;
    };
    match open.execute(command) {
        Ok(outcome) => Some(outcome),
        Err(error) => {
            state.error(format!("{}: {error}", command.label()));
            None
        }
    }
}

/// Selects exactly the nodes given in the current document.
pub fn select(state: &mut EditorState, nodes: &[Guid]) {
    if let Some(open) = state.document_mut() {
        open.selection_mut().set_all(nodes.iter().copied());
    }
}

/// Whether `ancestor` is above `node`.
fn is_below(document: &EntityDocument, node: Guid, ancestor: Guid) -> bool {
    let mut current = document.node_parent(node);
    while current.valid() {
        if current == ancestor {
            return true;
        }
        current = document.node_parent(current);
    }
    false
}

/// The selected nodes that have no selected ancestor, in selection order.
pub fn selected_roots(state: &EditorState) -> Vec<Guid> {
    let Some(open) = state.document() else {
        return Vec::new();
    };
    let content = open.document().content();
    let nodes = open.selection().nodes();
    nodes
        .iter()
        .copied()
        .filter(|node| !nodes.iter().any(|other| is_below(content, *node, *other)))
        .collect()
}

/// Deletes the selected nodes as one undo entry.
pub fn delete_selection(state: &mut EditorState) {
    let roots = selected_roots(state);
    if roots.is_empty() {
        return;
    }
    let commands: Vec<Command> = roots
        .into_iter()
        .map(|node| Command::DeleteNode { node })
        .collect();
    let label = if commands.len() == 1 {
        "Delete".to_owned()
    } else {
        format!("Delete {} Nodes", commands.len())
    };
    run(state, &Command::Batch { label, commands });
}

/// Duplicates the selected nodes as one undo entry and selects the copies.
pub fn duplicate_selection(state: &mut EditorState) {
    let roots = selected_roots(state);
    if roots.is_empty() {
        return;
    }
    let commands: Vec<Command> = roots
        .into_iter()
        .map(|node| Command::Duplicate { node })
        .collect();
    let label = if commands.len() == 1 {
        "Duplicate".to_owned()
    } else {
        format!("Duplicate {} Nodes", commands.len())
    };
    if let Some(outcome) = run(state, &Command::Batch { label, commands }) {
        select(state, &outcome.created);
    }
}

/// Starts typing over the name of the primary selected node in the Hierarchy.
pub fn begin_rename(state: &mut EditorState) {
    let Some(open) = state.document() else {
        return;
    };
    let Some(node) = open.selection().primary() else {
        return;
    };
    let content = open.document().content();
    let name = content
        .find_entity(node)
        .map(|entity| entity.name.clone())
        .or_else(|| {
            content
                .find_instance(node)
                .map(|instance| instance.name.clone())
        });
    if let Some(name) = name {
        state.hierarchy.renaming = Some(node);
        state.text.begin(TextTarget::HierarchyRename(node), &name);
    }
}

/// Undoes the last entry of the current document.
pub fn undo(state: &mut EditorState) {
    let Some(open) = state.document_mut() else {
        return;
    };
    match open.undo() {
        Ok(Some(label)) => state.toast = Some(format!("Undid {label}")),
        Ok(None) => {}
        Err(error) => state.error(format!("undo: {error}")),
    }
}

/// Redoes the last undone entry of the current document.
pub fn redo(state: &mut EditorState) {
    let Some(open) = state.document_mut() else {
        return;
    };
    match open.redo() {
        Ok(Some(label)) => state.toast = Some(format!("Redid {label}")),
        Ok(None) => {}
        Err(error) => state.error(format!("redo: {error}")),
    }
}
