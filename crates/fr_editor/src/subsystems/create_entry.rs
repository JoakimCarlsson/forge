//! One entry of the hierarchy's Create menu, contributed by a subsystem.

use fr_authoring::{Command, ComponentInit, CreateEntity};
use fr_document::Guid;
use fr_math::Vec3;
use fr_transform::Transform;
use fr_ui::IconName;

/// What the Create menu offers: a label and icon, and the entity it creates
/// with its default name, placement and components.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateEntry {
    /// The menu label.
    pub label: String,
    /// The group the entry is listed under.
    pub category: String,
    /// The icon beside the label.
    pub icon: IconName,
    /// The name the new entity takes.
    pub name: String,
    /// The transform the entity starts with, relative to its parent.
    pub transform: Transform,
    /// The components the entity is created with.
    pub components: Vec<ComponentInit>,
}

impl CreateEntry {
    /// An entry that creates an empty entity at the identity transform.
    pub fn new(
        label: impl Into<String>,
        category: impl Into<String>,
        icon: IconName,
        name: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            category: category.into(),
            icon,
            name: name.into(),
            transform: Transform::IDENTITY,
            components: Vec::new(),
        }
    }

    /// The same entry creating its entity at a transform.
    #[must_use]
    pub fn at(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    /// The same entry creating its entity with one more component.
    #[must_use]
    pub fn with_component(mut self, component: ComponentInit) -> Self {
        self.components.push(component);
        self
    }

    /// The command that creates the entity under `parent`, moved by `offset`
    /// from the entry's own placement.
    pub fn command(&self, parent: Guid, offset: Vec3) -> Command {
        let mut transform = self.transform;
        transform.translation += offset;
        let mut create = CreateEntity::new(self.name.clone())
            .under(parent)
            .at(transform);
        for component in &self.components {
            create = create.with_component(component.clone());
        }
        Command::CreateEntity(create)
    }
}
