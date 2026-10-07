//! Entity, component and prefab instance records, and the document holding
//! them.

use fr_transform::Transform;

use crate::guid::Guid;
use crate::property::{AssetReference, PropertyEntry, PropertyValue};

/// One component of an entity: its type, schema version and properties.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentRecord {
    /// The component identity.
    pub id: Guid,
    /// The registered component type key.
    pub kind: String,
    /// The schema version the properties were written against.
    pub schema_version: u32,
    /// Whether the component takes part in the scene.
    pub enabled: bool,
    /// The properties in authored order.
    pub properties: Vec<PropertyEntry>,
}

impl Default for ComponentRecord {
    /// An enabled component of schema version one with no type or properties.
    fn default() -> Self {
        Self {
            id: Guid::NONE,
            kind: String::new(),
            schema_version: 1,
            enabled: true,
            properties: Vec::new(),
        }
    }
}

impl ComponentRecord {
    /// The value of a property, when the record carries it.
    pub fn property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    }
}

/// An authored entity.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityRecord {
    /// The entity identity.
    pub id: Guid,
    /// The display name.
    pub name: String,
    /// The transform relative to the parent.
    pub transform: Transform,
    /// The parent node, unset for a root.
    pub parent: Guid,
    /// The position among siblings.
    pub order: i32,
    /// Whether the entity is active.
    pub active: bool,
    /// The groups the entity belongs to.
    pub groups: Vec<String>,
    /// The components in authored order.
    pub components: Vec<ComponentRecord>,
}

impl Default for EntityRecord {
    /// An active root entity with an identity transform.
    fn default() -> Self {
        Self {
            id: Guid::NONE,
            name: String::new(),
            transform: Transform::IDENTITY,
            parent: Guid::NONE,
            order: 0,
            active: true,
            groups: Vec::new(),
            components: Vec::new(),
        }
    }
}

impl EntityRecord {
    /// The first component of a type.
    pub fn find_component(&self, kind: &str) -> Option<&ComponentRecord> {
        self.components
            .iter()
            .find(|component| component.kind == kind)
    }
}

/// A value an instance replaces on one property of its prefab.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyOverride {
    /// The prefab entity the override applies to.
    pub entity: Guid,
    /// The component, unset for the entity's own name, transform or active flag.
    pub component: Guid,
    /// The property key.
    pub property: String,
    /// The replacement value.
    pub value: PropertyValue,
}

/// An instance of a prefab placed in a document.
#[derive(Clone, Debug, PartialEq)]
pub struct PrefabInstanceRecord {
    /// The instance identity.
    pub id: Guid,
    /// The display name.
    pub name: String,
    /// The transform relative to the parent.
    pub transform: Transform,
    /// The parent node, unset for a root.
    pub parent: Guid,
    /// The position among siblings.
    pub order: i32,
    /// Whether the instance is active.
    pub active: bool,
    /// The prefab asset.
    pub prefab: AssetReference,
    /// The property overrides.
    pub overrides: Vec<PropertyOverride>,
}

impl Default for PrefabInstanceRecord {
    /// An active root instance with an identity transform.
    fn default() -> Self {
        Self {
            id: Guid::NONE,
            name: String::new(),
            transform: Transform::IDENTITY,
            parent: Guid::NONE,
            order: 0,
            active: true,
            prefab: AssetReference::default(),
            overrides: Vec::new(),
        }
    }
}

/// The entities and prefab instances of a scene or prefab file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityDocument {
    /// The entities.
    pub entities: Vec<EntityRecord>,
    /// The prefab instances.
    pub instances: Vec<PrefabInstanceRecord>,
}
