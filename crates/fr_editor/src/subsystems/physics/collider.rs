//! The contribution for `forge.collider`.

use fr_authoring::ComponentInit;
use fr_color::Rgba;
use fr_document::PropertyValue;
use fr_document::builtin_schema::{COLLIDER, RIGID_BODY};
use fr_math::{Aabb, Vec3};
use fr_transform::Transform;
use fr_ui::IconName;

use crate::subsystems::mesh_renderer::primitive_entry;
use crate::subsystems::{BoundsContext, CreateEntry, GizmoSink, Properties, Subsystem};

/// The group of the physics entries in the Create menu.
const PHYSICS_CATEGORY: &str = "Physics";

/// The colour of a collider's wire shape.
const COLLIDER_COLOR: Rgba = Rgba::hex(0x7fe08a);

/// Contributes the collider: the extent of its shape for picking, its wire
/// shape and the rigid shapes of the Create menu.
pub struct ColliderSubsystem;

impl ColliderSubsystem {
    /// The collider shape as a creation spec.
    fn component(shape: &str) -> ComponentInit {
        ComponentInit::new(COLLIDER).with("shape", PropertyValue::Enum(shape.to_owned()))
    }

    /// The entry of a mesh primitive with a dynamic body and a collider of the
    /// matching shape.
    fn rigid_entry(label: &str, primitive: &str, shape: &str) -> CreateEntry {
        let entry = primitive_entry(label, primitive);
        CreateEntry {
            category: PHYSICS_CATEGORY.to_owned(),
            ..entry
        }
        .with_component(ComponentInit::new(RIGID_BODY))
        .with_component(Self::component(shape))
    }
}

/// The half size of the box that contains the collider's shape around its
/// offset.
fn shape_extents(component: &Properties<'_>) -> Vec3 {
    let radius = component.float("radius");
    match component.text("shape") {
        "sphere" => Vec3::splat(radius),
        "capsule" => Vec3::new(radius, component.float("half_height") + radius, radius),
        _ => component.vec3("half_extents"),
    }
}

impl Subsystem for ColliderSubsystem {
    /// The collider's type key.
    fn kind(&self) -> &'static str {
        COLLIDER
    }

    /// The box icon.
    fn icon(&self) -> IconName {
        IconName::Box
    }

    /// The box around the shape at its offset.
    fn bounds(&self, component: &Properties<'_>, _context: &mut BoundsContext<'_>) -> Option<Aabb> {
        Some(Aabb::from_center_extents(
            component.vec3("offset"),
            shape_extents(component),
        ))
    }

    /// The shape as wire geometry. A collider's size is its own, so the entity's
    /// scale is left out of the placement.
    fn gizmo(&self, component: &Properties<'_>, world: &Transform, sink: &mut GizmoSink) {
        sink.set_placement(Transform {
            scale: Vec3::ONE,
            ..*world
        });
        sink.set_color(COLLIDER_COLOR);
        let offset = component.vec3("offset");
        match component.text("shape") {
            "sphere" => sink.wire_sphere(offset, component.float("radius")),
            "capsule" => sink.wire_capsule(
                offset,
                component.float("radius"),
                component.float("half_height"),
            ),
            _ => sink.wire_box(offset, component.vec3("half_extents")),
        }
    }

    /// Rigid Box, Rigid Sphere and Rigid Capsule.
    fn create_entries(&self) -> Vec<CreateEntry> {
        vec![
            Self::rigid_entry("Rigid Box", "cube", "box"),
            Self::rigid_entry("Rigid Sphere", "sphere", "sphere"),
            Self::rigid_entry("Rigid Capsule", "capsule", "capsule"),
        ]
    }
}
