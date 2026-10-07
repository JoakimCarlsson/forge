//! The rows of the Inspector as data: the node's own rows, its transform and
//! each component, built from the document and the subsystem registry.
//!
//! The view and the update both call [`rows_of`], so an event is applied to a
//! row built from the document as it is now, and nothing here knows a component
//! type: the registry answers with a contribution's rows or the schema's.

use std::rc::Rc;

use fr_authoring::{Command, OpenDocument};
use fr_document::{ComponentRecord, EntityRecord, Guid, PrefabInstanceRecord};
use fr_transform::Transform;

use super::row_editor::RowOwner;
use crate::subsystems::{InspectorContext, InspectorRow, RowValue, SubsystemRegistry, no_change};

/// The key of the row that names the node.
const NAME_KEY: &str = "name";

/// The key of the row that activates the node.
const ACTIVE_KEY: &str = "active";

/// The key of the row that shows an instance's prefab.
const PREFAB_KEY: &str = "prefab";

/// An entity or a prefab instance of the document.
#[derive(Clone, Copy)]
pub enum Node<'a> {
    /// An entity record.
    Entity(&'a EntityRecord),
    /// A prefab instance record.
    Instance(&'a PrefabInstanceRecord),
}

impl<'a> Node<'a> {
    /// The node with an identity in the document.
    pub fn find(document: &'a OpenDocument, id: Guid) -> Option<Self> {
        let content = document.document().content();
        content
            .find_entity(id)
            .map(Self::Entity)
            .or_else(|| content.find_instance(id).map(Self::Instance))
    }

    /// The identity.
    pub fn id(self) -> Guid {
        match self {
            Self::Entity(entity) => entity.id,
            Self::Instance(instance) => instance.id,
        }
    }

    /// The display name.
    pub fn name(self) -> &'a str {
        match self {
            Self::Entity(entity) => &entity.name,
            Self::Instance(instance) => &instance.name,
        }
    }

    /// Whether the node is active.
    pub fn active(self) -> bool {
        match self {
            Self::Entity(entity) => entity.active,
            Self::Instance(instance) => instance.active,
        }
    }

    /// The transform relative to the parent.
    pub fn transform(self) -> Transform {
        match self {
            Self::Entity(entity) => entity.transform,
            Self::Instance(instance) => instance.transform,
        }
    }

    /// The entity record, when the node is an entity.
    pub fn entity(self) -> Option<&'a EntityRecord> {
        match self {
            Self::Entity(entity) => Some(entity),
            Self::Instance(_) => None,
        }
    }
}

/// A row that stores its value through one command built from it.
fn command_row(
    key: &str,
    label: &str,
    value: RowValue,
    read_only: bool,
    apply: impl Fn(RowValue) -> Option<Command> + 'static,
) -> InspectorRow {
    InspectorRow {
        key: key.to_owned(),
        label: label.to_owned(),
        value,
        range: None,
        step: None,
        read_only,
        apply: Rc::new(move |value| apply(value).unwrap_or_else(no_change)),
    }
}

/// The name, activity and, for an instance, prefab of a node. A locked name
/// is shown without being editable.
pub fn header_rows(node: Node<'_>, lock_name: bool) -> Vec<InspectorRow> {
    let id = node.id();
    let mut rows = vec![
        command_row(
            NAME_KEY,
            "Name",
            RowValue::Text(node.name().to_owned()),
            lock_name,
            move |value| match value {
                RowValue::Text(name) => Some(Command::Rename { node: id, name }),
                _ => None,
            },
        ),
        command_row(
            ACTIVE_KEY,
            "Active",
            RowValue::Bool(node.active()),
            false,
            move |value| match value {
                RowValue::Bool(active) => Some(Command::SetActive { node: id, active }),
                _ => None,
            },
        ),
    ];
    if let Node::Instance(instance) = node {
        rows.push(command_row(
            PREFAB_KEY,
            "Prefab",
            RowValue::Asset(instance.prefab.clone()),
            true,
            |_| None,
        ));
    }
    rows
}

/// The transform rows of a node, which the registry's transform contribution
/// supplies.
pub fn transform_rows(
    registry: &SubsystemRegistry,
    document: &OpenDocument,
    node: Node<'_>,
) -> Vec<InspectorRow> {
    registry.inspector_rows(&InspectorContext {
        node: node.id(),
        entity: node.entity(),
        component: None,
        transform: node.transform(),
        schemas: document.document().schemas(),
    })
}

/// The rows of one component of an entity.
pub fn component_rows(
    registry: &SubsystemRegistry,
    document: &OpenDocument,
    entity: &EntityRecord,
    component: &ComponentRecord,
) -> Vec<InspectorRow> {
    registry.inspector_rows(&InspectorContext {
        node: entity.id,
        entity: Some(entity),
        component: Some(component),
        transform: entity.transform,
        schemas: document.document().schemas(),
    })
}

/// The rows an owner has in the document as it is now: those of the node, of a
/// component of the node, or of the scene settings.
pub fn rows_of(
    registry: &SubsystemRegistry,
    document: &OpenDocument,
    node: Option<Guid>,
    owner: RowOwner,
    lock_name: bool,
) -> Vec<InspectorRow> {
    match owner {
        RowOwner::Scene => super::inspector_scene::scene_rows(document),
        RowOwner::Node => node
            .and_then(|id| Node::find(document, id))
            .map(|node| {
                let mut rows = header_rows(node, lock_name);
                rows.extend(transform_rows(registry, document, node));
                rows
            })
            .unwrap_or_default(),
        RowOwner::Component(id) => node
            .and_then(|node| document.document().content().find_entity(node))
            .and_then(|entity| {
                entity
                    .components
                    .iter()
                    .find(|component| component.id == id)
                    .map(|component| component_rows(registry, document, entity, component))
            })
            .unwrap_or_default(),
    }
}
