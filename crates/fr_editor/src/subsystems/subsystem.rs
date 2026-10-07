//! The contribution a subsystem makes for one component type.

use fr_math::Aabb;
use fr_transform::Transform;
use fr_ui::IconName;

use super::context::BoundsContext;
use super::create_entry::CreateEntry;
use super::gizmo_sink::GizmoSink;
use super::inspector::InspectorContribution;
use super::properties::Properties;

/// What the editor knows about one component type, found by the type's stable
/// key so no shared code dispatches on an enumeration of types. Only the icon
/// is required; everything else defaults to contributing nothing.
pub trait Subsystem {
    /// The component type key this contribution is for. The transform, which is
    /// not a component record, is described by `fr_document::builtin_schema::TRANSFORM`.
    fn kind(&self) -> &'static str;

    /// The icon of the type in the hierarchy and the inspector.
    fn icon(&self) -> IconName;

    /// The bounds of the component in the local space of its entity, which
    /// picking and framing use; none when the component has no extent.
    fn bounds(
        &self,
        _component: &Properties<'_>,
        _context: &mut BoundsContext<'_>,
    ) -> Option<Aabb> {
        None
    }

    /// Draws the component's handles into `sink` for an entity at the world
    /// transform `world`.
    fn gizmo(&self, _component: &Properties<'_>, _world: &Transform, _sink: &mut GizmoSink) {}

    /// The entries this type adds to the Create menu.
    fn create_entries(&self) -> Vec<CreateEntry> {
        Vec::new()
    }

    /// The specialised inspector of the type; none for the generic rows of its
    /// schema.
    fn inspector(&self) -> Option<&dyn InspectorContribution> {
        None
    }
}
