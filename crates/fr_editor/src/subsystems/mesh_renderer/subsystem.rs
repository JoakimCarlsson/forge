//! The contribution for `forge.mesh_renderer`.

use std::rc::Rc;

use fr_authoring::ComponentInit;
use fr_document::PropertyValue;
use fr_document::builtin_schema::MESH_RENDERER;
use fr_math::{Aabb, Vec3};
use fr_ui::IconName;

use crate::subsystems::{BoundsContext, CreateEntry, Properties, Subsystem, SubsystemRegistry};

/// The group of the entries of the mesh renderer in the Create menu.
const OBJECT_CATEGORY: &str = "3D Object";

/// Adds the mesh renderer subsystem to the registry.
pub fn register(registry: &mut SubsystemRegistry) {
    registry.add(Rc::new(MeshRendererSubsystem));
}

/// The mesh renderer component of a primitive, as a creation spec.
pub fn primitive_component(primitive: &str) -> ComponentInit {
    ComponentInit::new(MESH_RENDERER).with("primitive", PropertyValue::Enum(primitive.to_owned()))
}

/// The Create entry of an entity drawn as a primitive.
pub fn primitive_entry(label: &str, primitive: &str) -> CreateEntry {
    CreateEntry::new(label, OBJECT_CATEGORY, IconName::Cube, label)
        .with_component(primitive_component(primitive))
}

/// Contributes the mesh renderer.
struct MeshRendererSubsystem;

impl Subsystem for MeshRendererSubsystem {
    /// The mesh renderer's type key.
    fn kind(&self) -> &'static str {
        MESH_RENDERER
    }

    /// The cube icon.
    fn icon(&self) -> IconName {
        IconName::Cube
    }

    /// The bounds of the model, or the extent of the primitive: a unit cube, a
    /// sphere of diameter one, or a capsule of its radius and core length along
    /// Y.
    fn bounds(&self, component: &Properties<'_>, context: &mut BoundsContext<'_>) -> Option<Aabb> {
        let model = component.asset("model");
        if model.asset.valid() {
            return context.model_bounds(model.asset);
        }
        let half = match component.text("primitive") {
            "cube" | "sphere" => Vec3::splat(0.5),
            "capsule" => {
                let radius = component.float("radius");
                Vec3::new(radius, component.float("length") * 0.5 + radius, radius)
            }
            _ => return None,
        };
        Some(Aabb::from_center_extents(Vec3::ZERO, half))
    }

    /// Cube, Sphere and Capsule.
    fn create_entries(&self) -> Vec<CreateEntry> {
        vec![
            primitive_entry("Cube", "cube"),
            primitive_entry("Sphere", "sphere"),
            primitive_entry("Capsule", "capsule"),
        ]
    }
}
