//! The rows of the Hierarchy as plain data: which nodes are listed, how deep,
//! and whether each is expanded. Pure over the document so it can be checked
//! without a window.

use std::collections::HashSet;

use fr_document::{EntityDocument, Guid, NodeKind};

/// One listed node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The node.
    pub node: Guid,
    /// Whether it is an entity or a prefab instance.
    pub kind: NodeKind,
    /// How many ancestors it has.
    pub depth: usize,
    /// Its name.
    pub name: String,
    /// The type key of its first component, which picks its icon.
    pub first_component: Option<String>,
    /// Whether it takes part in the scene.
    pub active: bool,
    /// Whether it has children, so it can be expanded.
    pub has_children: bool,
    /// Whether its children are listed.
    pub expanded: bool,
}

/// Whether `name` contains `filter`, ignoring case; an empty filter matches.
fn matches(name: &str, filter: &str) -> bool {
    filter.is_empty() || name.to_lowercase().contains(&filter.to_lowercase())
}

/// The name, first component and activity of a node.
fn describe(document: &EntityDocument, node: Guid) -> (String, Option<String>, bool) {
    if let Some(entity) = document.find_entity(node) {
        let first = entity
            .components
            .first()
            .map(|component| component.kind.clone());
        return (entity.name.clone(), first, entity.active);
    }
    document
        .find_instance(node)
        .map_or((String::new(), None, true), |instance| {
            (instance.name.clone(), None, instance.active)
        })
}

/// Whether the node or anything below it matches the filter.
fn subtree_matches(document: &EntityDocument, node: Guid, filter: &str) -> bool {
    let (name, _, _) = describe(document, node);
    matches(&name, filter)
        || document
            .child_nodes(node)
            .iter()
            .any(|child| subtree_matches(document, child.id, filter))
}

/// Appends the rows below `parent`.
fn append(
    document: &EntityDocument,
    parent: Guid,
    depth: usize,
    collapsed: &HashSet<Guid>,
    filter: &str,
    out: &mut Vec<Row>,
) {
    for child in document.child_nodes(parent) {
        if !subtree_matches(document, child.id, filter) {
            continue;
        }
        let (name, first_component, active) = describe(document, child.id);
        let has_children = !document.child_nodes(child.id).is_empty();
        let expanded = !filter.is_empty() || !collapsed.contains(&child.id);
        out.push(Row {
            node: child.id,
            kind: child.kind,
            depth,
            name,
            first_component,
            active,
            has_children,
            expanded,
        });
        if has_children && expanded {
            append(document, child.id, depth + 1, collapsed, filter, out);
        }
    }
}

/// The listed rows below the document's own row, in order. A filter lists the
/// matching nodes with their ancestors and expands everything.
pub fn visible_rows(
    document: &EntityDocument,
    collapsed: &HashSet<Guid>,
    filter: &str,
) -> Vec<Row> {
    let mut rows = Vec::new();
    append(document, Guid::NONE, 1, collapsed, filter, &mut rows);
    rows
}
