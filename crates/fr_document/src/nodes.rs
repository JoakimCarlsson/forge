//! Queries over the nodes of an entity document, and the resolution of a prefab
//! instance's overrides.

use crate::guid::Guid;
use crate::records::{EntityDocument, EntityRecord, PrefabInstanceRecord, PropertyOverride};

/// Deepest parent chain any walk follows.
pub const MAX_DEPTH: i32 = 4096;

/// Whether a node is an entity or a prefab instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// An entity record.
    Entity,
    /// A prefab instance record.
    Instance,
}

/// A node of a document: its kind and identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeRef {
    /// Whether the node is an entity or an instance.
    pub kind: NodeKind,
    /// The node identity.
    pub id: Guid,
}

impl EntityDocument {
    /// The entity with an identity.
    pub fn find_entity(&self, id: Guid) -> Option<&EntityRecord> {
        self.entities.iter().find(|entity| entity.id == id)
    }

    /// The prefab instance with an identity.
    pub fn find_instance(&self, id: Guid) -> Option<&PrefabInstanceRecord> {
        self.instances.iter().find(|instance| instance.id == id)
    }

    /// Whether a node with the identity exists.
    pub fn has_node(&self, id: Guid) -> bool {
        self.find_entity(id).is_some() || self.find_instance(id).is_some()
    }

    /// The parent of a node; unset when absent or a root.
    pub fn node_parent(&self, id: Guid) -> Guid {
        if let Some(entity) = self.find_entity(id) {
            return entity.parent;
        }
        self.find_instance(id)
            .map_or(Guid::NONE, |instance| instance.parent)
    }

    /// The number of ancestors of a node.
    pub fn node_depth(&self, node: Guid) -> i32 {
        let mut depth = 0;
        let mut current = self.node_parent(node);
        while current.valid() && depth < MAX_DEPTH {
            depth += 1;
            current = self.node_parent(current);
        }
        depth
    }

    /// The children of a parent in sibling order, entities before instances on
    /// a tie; an unset parent selects the roots.
    pub fn child_nodes(&self, parent: Guid) -> Vec<NodeRef> {
        let mut children: Vec<(i32, NodeRef)> = Vec::new();
        for entity in self
            .entities
            .iter()
            .filter(|entity| entity.parent == parent)
        {
            children.push((
                entity.order,
                NodeRef {
                    kind: NodeKind::Entity,
                    id: entity.id,
                },
            ));
        }
        for instance in self
            .instances
            .iter()
            .filter(|instance| instance.parent == parent)
        {
            children.push((
                instance.order,
                NodeRef {
                    kind: NodeKind::Instance,
                    id: instance.id,
                },
            ));
        }
        children.sort_by_key(|(order, _)| *order);
        children.into_iter().map(|(_, node)| node).collect()
    }

    /// Every node reachable from the roots, depth first in sibling order, each
    /// parent before its children. A node in a parent cycle or under a missing
    /// parent is not reachable, which validation reports.
    pub fn ordered_nodes(&self) -> Vec<NodeRef> {
        let mut order = Vec::with_capacity(self.entities.len() + self.instances.len());
        self.append_descendants(Guid::NONE, &mut order, 0);
        order
    }

    /// Appends the descendants of a parent, each before its own children.
    fn append_descendants(&self, parent: Guid, out: &mut Vec<NodeRef>, depth: i32) {
        if depth > MAX_DEPTH {
            return;
        }
        for child in self.child_nodes(parent) {
            out.push(child);
            self.append_descendants(child.id, out, depth + 1);
        }
    }
}

/// The override an instance holds for one property.
pub fn find_override<'a>(
    instance: &'a PrefabInstanceRecord,
    entity: Guid,
    component: Guid,
    property: &str,
) -> Option<&'a PropertyOverride> {
    instance.overrides.iter().find(|candidate| {
        candidate.entity == entity
            && candidate.component == component
            && candidate.property == property
    })
}

/// A prefab entity with an instance's overrides applied; a copy when there is
/// none. An override replaces a value only when it has the property's type.
pub fn effective_entity(
    source: &EntityRecord,
    instance: Option<&PrefabInstanceRecord>,
) -> EntityRecord {
    let mut result = source.clone();
    let Some(instance) = instance else {
        return result;
    };
    if let Some(found) = find_override(instance, source.id, Guid::NONE, "name")
        && let Some(text) = found.value.as_text()
    {
        text.clone_into(&mut result.name);
    }
    if let Some(found) = find_override(instance, source.id, Guid::NONE, "transform")
        && let crate::PropertyValue::Transform(transform) = &found.value
    {
        result.transform = *transform;
    }
    if let Some(found) = find_override(instance, source.id, Guid::NONE, "active")
        && let Some(active) = found.value.as_bool()
    {
        result.active = active;
    }
    for component in &mut result.components {
        for entry in &mut component.properties {
            if let Some(found) = find_override(instance, source.id, component.id, &entry.key)
                && found.value.kind() == entry.value.kind()
            {
                entry.value = found.value.clone();
            }
        }
    }
    result
}
