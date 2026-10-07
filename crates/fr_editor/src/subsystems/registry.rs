//! The registry of subsystems and the one place that names the built-in ones.

use std::rc::Rc;

use fr_document::builtin_schema::TRANSFORM;
use fr_document::{ComponentRecord, SchemaSet};
use fr_math::Aabb;
use fr_transform::Transform;
use fr_ui::IconName;

use super::context::BoundsContext;
use super::create_entry::CreateEntry;
use super::gizmo_sink::GizmoSink;
use super::inspector::{InspectorContext, InspectorRow, generic_rows};
use super::properties::Properties;
use super::subsystem::Subsystem;
use super::{camera, light, mesh_renderer, physics, transform};

/// The icon of a component type no subsystem contributes.
const FALLBACK_ICON: IconName = IconName::Box;

/// The contributions of every registered subsystem, in registration order,
/// which is the order of the Create menu.
#[derive(Clone, Default)]
pub struct SubsystemRegistry {
    /// The contributions in registration order.
    subsystems: Vec<Rc<dyn Subsystem>>,
}

impl SubsystemRegistry {
    /// A registry with nothing in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry with every built-in subsystem registered.
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        register_builtin_subsystems(&mut registry);
        registry
    }

    /// Registers a contribution. The first contribution registered for a type
    /// key answers for it.
    pub fn add(&mut self, subsystem: Rc<dyn Subsystem>) {
        self.subsystems.push(subsystem);
    }

    /// Every contribution in registration order.
    pub fn subsystems(&self) -> &[Rc<dyn Subsystem>] {
        &self.subsystems
    }

    /// The contribution for a component type key.
    pub fn find(&self, kind: &str) -> Option<&dyn Subsystem> {
        self.subsystems
            .iter()
            .find(|subsystem| subsystem.kind() == kind)
            .map(AsRef::as_ref)
    }

    /// The icon of a component type, or a box for a type nobody contributes.
    pub fn icon(&self, kind: &str) -> IconName {
        self.find(kind)
            .map_or(FALLBACK_ICON, |subsystem| subsystem.icon())
    }

    /// The local bounds of a component; none when its type has no contribution,
    /// no schema is available for it or it has no extent.
    pub fn bounds(
        &self,
        record: &ComponentRecord,
        schemas: &SchemaSet,
        context: &mut BoundsContext<'_>,
    ) -> Option<Aabb> {
        let subsystem = self.find(&record.kind)?;
        let schema = schemas.find(&record.kind)?;
        subsystem.bounds(&Properties::new(record, schema), context)
    }

    /// Draws the gizmo of a component into `sink` for an entity at the world
    /// transform `world`; nothing for a type without a contribution.
    pub fn gizmo(
        &self,
        record: &ComponentRecord,
        schemas: &SchemaSet,
        world: &Transform,
        sink: &mut GizmoSink,
    ) {
        let (Some(subsystem), Some(schema)) = (self.find(&record.kind), schemas.find(&record.kind))
        else {
            return;
        };
        subsystem.gizmo(&Properties::new(record, schema), world, sink);
    }

    /// The entries of the Create menu, in registration order.
    pub fn create_entries(&self) -> Vec<CreateEntry> {
        self.subsystems
            .iter()
            .flat_map(|subsystem| subsystem.create_entries())
            .collect()
    }

    /// The rows of a component, or of the node's transform when the context has
    /// no component: the type's own inspector when it has one, otherwise the
    /// generic rows of the schema.
    pub fn inspector_rows(&self, context: &InspectorContext<'_>) -> Vec<InspectorRow> {
        let kind = context
            .component
            .map_or(TRANSFORM, |record| record.kind.as_str());
        let specialised = self
            .find(kind)
            .and_then(|subsystem| subsystem.inspector())
            .map(|inspector| inspector.rows(context));
        match (specialised, context.component) {
            (Some(rows), _) => rows,
            (None, Some(record)) => generic_rows(context.node, record, context.schemas),
            (None, None) => Vec::new(),
        }
    }
}

/// Registers every built-in subsystem. This is the one place that names the
/// subsystem folders: a new subsystem is one folder under `subsystems/` with a
/// `register` function and one line here.
pub fn register_builtin_subsystems(registry: &mut SubsystemRegistry) {
    transform::register(registry);
    mesh_renderer::register(registry);
    camera::register(registry);
    light::register(registry);
    physics::register(registry);
}
