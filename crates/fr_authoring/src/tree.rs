//! Structural helpers over the hierarchy of an entity document: subtrees,
//! sibling order, field access shared by entities and instances, and copying a
//! subtree with new identities.

use std::collections::{HashMap, HashSet};

use fr_document::{
    ComponentReference, EntityDocument, EntityReference, Guid, NodeKind, PropertyValue,
};
use fr_transform::Transform;

use crate::error::AuthoringError;

/// Mutable access to the fields an entity and a prefab instance share.
pub struct NodeFieldsMut<'a> {
    /// The display name.
    pub name: &'a mut String,
    /// The transform relative to the parent.
    pub transform: &'a mut Transform,
    /// The parent node.
    pub parent: &'a mut Guid,
    /// The position among siblings.
    pub order: &'a mut i32,
    /// Whether the node is active.
    pub active: &'a mut bool,
}

/// The shared fields of a node.
///
/// # Errors
///
/// [`AuthoringError::NodeNotFound`] when the document has no such node.
pub fn node_fields_mut(
    document: &mut EntityDocument,
    id: Guid,
) -> Result<NodeFieldsMut<'_>, AuthoringError> {
    if let Some(entity) = document.entities.iter_mut().find(|entity| entity.id == id) {
        return Ok(NodeFieldsMut {
            name: &mut entity.name,
            transform: &mut entity.transform,
            parent: &mut entity.parent,
            order: &mut entity.order,
            active: &mut entity.active,
        });
    }
    document
        .instances
        .iter_mut()
        .find(|instance| instance.id == id)
        .map(|instance| NodeFieldsMut {
            name: &mut instance.name,
            transform: &mut instance.transform,
            parent: &mut instance.parent,
            order: &mut instance.order,
            active: &mut instance.active,
        })
        .ok_or(AuthoringError::NodeNotFound { id })
}

/// Checks that a node exists.
///
/// # Errors
///
/// [`AuthoringError::NodeNotFound`] when it does not.
pub fn require_node(document: &EntityDocument, id: Guid) -> Result<(), AuthoringError> {
    if document.has_node(id) {
        Ok(())
    } else {
        Err(AuthoringError::NodeNotFound { id })
    }
}

/// Checks that a parent is unset or an existing node.
///
/// # Errors
///
/// [`AuthoringError::NodeNotFound`] when it is set and missing.
pub fn require_parent(document: &EntityDocument, parent: Guid) -> Result<(), AuthoringError> {
    if parent.valid() {
        require_node(document, parent)
    } else {
        Ok(())
    }
}

/// A node and all its descendants, each parent before its children.
pub fn subtree_ids(document: &EntityDocument, root: Guid) -> Vec<Guid> {
    let mut found = vec![root];
    let mut seen: HashSet<Guid> = HashSet::from([root]);
    let mut cursor = 0;
    while cursor < found.len() {
        let current = found[cursor];
        cursor += 1;
        for child in document.child_nodes(current) {
            if seen.insert(child.id) {
                found.push(child.id);
            }
        }
    }
    found
}

/// Puts a node under a parent at a position among that parent's children and
/// renumbers the siblings consecutively. A position past the end, or none,
/// appends.
///
/// # Errors
///
/// [`AuthoringError::NodeNotFound`] when the node is missing.
pub fn place(
    document: &mut EntityDocument,
    node: Guid,
    parent: Guid,
    index: Option<usize>,
) -> Result<(), AuthoringError> {
    *node_fields_mut(document, node)?.parent = parent;
    let mut siblings: Vec<Guid> = document
        .child_nodes(parent)
        .into_iter()
        .map(|child| child.id)
        .filter(|id| *id != node)
        .collect();
    let position = index.map_or(siblings.len(), |wanted| wanted.min(siblings.len()));
    siblings.insert(position, node);
    for (order, id) in siblings.into_iter().enumerate() {
        *node_fields_mut(document, id)?.order = i32::try_from(order).unwrap_or(i32::MAX);
    }
    Ok(())
}

/// The position of a node among its parent's children.
pub fn sibling_index(document: &EntityDocument, node: Guid) -> Option<usize> {
    document
        .child_nodes(document.node_parent(node))
        .iter()
        .position(|child| child.id == node)
}

/// Removes a node and all its descendants, returning how many nodes went.
///
/// # Errors
///
/// [`AuthoringError::NodeNotFound`] when the node is missing.
pub fn remove_subtree(document: &mut EntityDocument, node: Guid) -> Result<usize, AuthoringError> {
    require_node(document, node)?;
    let doomed: HashSet<Guid> = subtree_ids(document, node).into_iter().collect();
    document
        .entities
        .retain(|entity| !doomed.contains(&entity.id));
    document
        .instances
        .retain(|instance| !doomed.contains(&instance.id));
    Ok(doomed.len())
}

/// Copies a node and its subtree with new identities for every node and
/// component, places the copy right after the original and returns its root.
/// References between copied entities point at the copies. The root is named
/// "Name (copy)".
///
/// # Errors
///
/// [`AuthoringError::NodeNotFound`] when the node is missing.
pub fn duplicate_subtree(
    document: &mut EntityDocument,
    node: Guid,
) -> Result<Guid, AuthoringError> {
    require_node(document, node)?;
    let ids = subtree_ids(document, node);
    let nodes: HashMap<Guid, Guid> = ids.iter().map(|id| (*id, Guid::generate())).collect();
    let components = fresh_component_ids(document, &ids);
    let original_parent = document.node_parent(node);
    let after = sibling_index(document, node).map(|index| index + 1);
    let mut entities = Vec::new();
    let mut instances = Vec::new();
    for id in &ids {
        let new_id = nodes[id];
        if let Some(source) = document.find_entity(*id) {
            let mut copy = source.clone();
            copy.id = new_id;
            copy.parent = nodes.get(&source.parent).copied().unwrap_or(source.parent);
            for component in &mut copy.components {
                component.id = components
                    .get(&component.id)
                    .copied()
                    .unwrap_or(component.id);
                for entry in &mut component.properties {
                    remap_references(&mut entry.value, &nodes, &components);
                }
            }
            entities.push(copy);
        } else if let Some(source) = document.find_instance(*id) {
            let mut copy = source.clone();
            copy.id = new_id;
            copy.parent = nodes.get(&source.parent).copied().unwrap_or(source.parent);
            instances.push(copy);
        }
    }
    let root = nodes[&node];
    document.entities.extend(entities);
    document.instances.extend(instances);
    let fields = node_fields_mut(document, root)?;
    *fields.name = format!("{} (copy)", fields.name);
    place(document, root, original_parent, after)?;
    Ok(root)
}

/// A new identity for every component of the given entities, keyed by the old
/// one.
fn fresh_component_ids(document: &EntityDocument, nodes: &[Guid]) -> HashMap<Guid, Guid> {
    nodes
        .iter()
        .filter_map(|id| document.find_entity(*id))
        .flat_map(|entity| entity.components.iter())
        .map(|component| (component.id, Guid::generate()))
        .collect()
}

/// Points references to copied entities and components at the copies; a
/// reference through a prefab instance chain is left alone.
fn remap_references(
    value: &mut PropertyValue,
    nodes: &HashMap<Guid, Guid>,
    components: &HashMap<Guid, Guid>,
) {
    match value {
        PropertyValue::Entity(reference) => remap_entity(reference, nodes),
        PropertyValue::Component(ComponentReference { owner, component }) => {
            remap_entity(owner, nodes);
            if let Some(copy) = components.get(component) {
                *component = *copy;
            }
        }
        PropertyValue::Array(items) => {
            for item in items {
                remap_references(item, nodes, components);
            }
        }
        _ => {}
    }
}

/// Points a same-document entity reference at the copy of its target.
fn remap_entity(reference: &mut EntityReference, nodes: &HashMap<Guid, Guid>) {
    if reference.instances.is_empty()
        && let Some(copy) = nodes.get(&reference.entity)
    {
        reference.entity = *copy;
    }
}

/// The kind of a node, when it exists.
pub fn node_kind(document: &EntityDocument, id: Guid) -> Option<NodeKind> {
    if document.find_entity(id).is_some() {
        Some(NodeKind::Entity)
    } else if document.find_instance(id).is_some() {
        Some(NodeKind::Instance)
    } else {
        None
    }
}
