//! The commands that edit a document, each one undo entry.

use fr_document::{AssetReference, Guid, PropertyValue, SceneSettings};
use fr_transform::Transform;

/// A component to create together with an entity: its type and the properties
/// that differ from the schema's defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentInit {
    /// The component type key.
    pub kind: String,
    /// Property values replacing the defaults, in the order they are applied.
    pub properties: Vec<(String, PropertyValue)>,
}

impl ComponentInit {
    /// A component of a type with its default properties.
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            properties: Vec::new(),
        }
    }

    /// The same component with one property set.
    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.properties.push((key.into(), value));
        self
    }
}

/// Creates an entity.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateEntity {
    /// The display name.
    pub name: String,
    /// The parent node; unset for a root.
    pub parent: Guid,
    /// The position among the parent's children; none appends.
    pub index: Option<usize>,
    /// The transform relative to the parent.
    pub transform: Transform,
    /// The initial components; missing prerequisites are added before each.
    pub components: Vec<ComponentInit>,
}

impl CreateEntity {
    /// An empty root entity at the identity transform.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parent: Guid::NONE,
            index: None,
            transform: Transform::IDENTITY,
            components: Vec::new(),
        }
    }

    /// The same entity under a parent.
    #[must_use]
    pub fn under(mut self, parent: Guid) -> Self {
        self.parent = parent;
        self
    }

    /// The same entity at a transform.
    #[must_use]
    pub fn at(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// The same entity with one more initial component.
    #[must_use]
    pub fn with_component(mut self, component: ComponentInit) -> Self {
        self.components.push(component);
        self
    }
}

/// Places an instance of a prefab asset.
#[derive(Clone, Debug, PartialEq)]
pub struct AddPrefabInstance {
    /// The prefab asset; its identity must be set.
    pub prefab: AssetReference,
    /// The display name; empty takes the prefab's file stem.
    pub name: String,
    /// The parent node; unset for a root.
    pub parent: Guid,
    /// The position among the parent's children; none appends.
    pub index: Option<usize>,
    /// The transform relative to the parent.
    pub transform: Transform,
}

/// One edit of a document. Running a command produces exactly one history
/// entry, labelled by [`Command::label`].
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Creates an entity with its initial components.
    CreateEntity(CreateEntity),
    /// Deletes an entity or prefab instance with all its descendants.
    DeleteNode {
        /// The node to delete.
        node: Guid,
    },
    /// Copies a node and its subtree with new identities, naming the copy
    /// "Name (copy)" and placing it after the original.
    Duplicate {
        /// The node to copy.
        node: Guid,
    },
    /// Moves a node under another parent.
    Reparent {
        /// The node to move.
        node: Guid,
        /// The new parent; unset for a root.
        parent: Guid,
        /// The position among the new parent's children; none appends.
        index: Option<usize>,
    },
    /// Renames a node.
    Rename {
        /// The node to rename.
        node: Guid,
        /// The new name.
        name: String,
    },
    /// Sets the transform of an entity or prefab instance.
    SetTransform {
        /// The node to place.
        node: Guid,
        /// The transform relative to the parent.
        transform: Transform,
    },
    /// Adds a component, with the components it requires, to an entity.
    AddComponent {
        /// The entity.
        entity: Guid,
        /// The component type key.
        kind: String,
    },
    /// Removes a component from an entity.
    RemoveComponent {
        /// The entity.
        entity: Guid,
        /// The component identity.
        component: Guid,
    },
    /// Sets one property of a component.
    SetProperty {
        /// The entity.
        entity: Guid,
        /// The component identity.
        component: Guid,
        /// The property key.
        key: String,
        /// The new value.
        value: PropertyValue,
    },
    /// Enables or disables a component.
    SetComponentEnabled {
        /// The entity.
        entity: Guid,
        /// The component identity.
        component: Guid,
        /// Whether the component takes part in the scene.
        enabled: bool,
    },
    /// Activates or deactivates an entity or prefab instance.
    SetActive {
        /// The node.
        node: Guid,
        /// Whether the node is active.
        active: bool,
    },
    /// Places an instance of a prefab.
    AddPrefabInstance(AddPrefabInstance),
    /// Replaces the scene settings.
    SetSceneSettings(SceneSettings),
    /// Several commands applied in order as one entry, such as deleting a
    /// selection. The label is given; the batch fails as a whole.
    Batch {
        /// The label of the entry.
        label: String,
        /// The commands in the order they run.
        commands: Vec<Command>,
    },
}

impl Command {
    /// The label of the undo entry the command produces.
    pub fn label(&self) -> String {
        match self {
            Self::CreateEntity(_) => "Create Entity".to_owned(),
            Self::DeleteNode { .. } => "Delete".to_owned(),
            Self::Duplicate { .. } => "Duplicate".to_owned(),
            Self::Reparent { .. } => "Reparent".to_owned(),
            Self::Rename { .. } => "Rename".to_owned(),
            Self::SetTransform { .. } => "Set Transform".to_owned(),
            Self::AddComponent { kind, .. } => format!("Add {kind}"),
            Self::RemoveComponent { .. } => "Remove Component".to_owned(),
            Self::SetProperty { key, .. } => format!("Set {key}"),
            Self::SetComponentEnabled { enabled, .. } => if *enabled {
                "Enable Component"
            } else {
                "Disable Component"
            }
            .to_owned(),
            Self::SetActive { active, .. } => {
                if *active { "Activate" } else { "Deactivate" }.to_owned()
            }
            Self::AddPrefabInstance(_) => "Add Prefab Instance".to_owned(),
            Self::SetSceneSettings(_) => "Set Scene Settings".to_owned(),
            Self::Batch { label, .. } => label.clone(),
        }
    }

    /// The key under which consecutive runs of this command merge into one
    /// entry; only property and transform edits have one.
    pub fn coalesce_key(&self) -> Option<CoalesceKey> {
        match self {
            Self::SetProperty {
                component,
                key,
                entity,
                ..
            } => Some(CoalesceKey::new(*entity, *component, key)),
            Self::SetTransform { node, .. } => {
                Some(CoalesceKey::new(*node, Guid::NONE, "transform"))
            }
            _ => None,
        }
    }
}

/// Identifies what a coalescing edit changes, so that consecutive edits of the
/// same value become one undo entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoalesceKey {
    /// The node edited.
    pub node: Guid,
    /// The component edited; unset for the node's own fields.
    pub component: Guid,
    /// The property key, or "transform".
    pub property: String,
}

impl CoalesceKey {
    /// A key for a property of a node's component.
    pub fn new(node: Guid, component: Guid, property: &str) -> Self {
        Self {
            node,
            component,
            property: property.to_owned(),
        }
    }
}

/// What a command did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandOutcome {
    /// The identities the command created: the new root node of a create,
    /// duplicate or instance command, or the components an add created with
    /// the requested one last.
    pub created: Vec<Guid>,
    /// Whether the content changed; an unchanged result records no entry.
    pub changed: bool,
}
