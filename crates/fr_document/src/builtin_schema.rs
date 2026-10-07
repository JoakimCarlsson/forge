//! The schemas of the engine's built-in component types.
//!
//! Each type declares a stable namespaced key, a schema version, stable
//! property keys, defaults, multiplicity and dependency rules. The stage
//! registers its adapters against these keys.

use fr_math::Vec3;

use crate::property::Color;
use crate::schema::{ComponentSchema, PropertySchema, SchemaSet};

/// The type key of an entity's transform, which lives in the entity record.
pub const TRANSFORM: &str = "forge.transform";

/// The type key of an entity's name, which lives in the entity record.
pub const NAME: &str = "forge.name";

/// The type key of the component that draws a model or primitive.
pub const MESH_RENDERER: &str = "forge.mesh_renderer";

/// The type key of a camera.
pub const CAMERA: &str = "forge.camera";

/// The type key of a directional, point or spot light.
pub const LIGHT: &str = "forge.light";

/// The type key of a rigid body.
pub const RIGID_BODY: &str = "forge.rigid_body";

/// The type key of a collider.
pub const COLLIDER: &str = "forge.collider";

/// The type key of a humanoid ragdoll.
pub const RAGDOLL: &str = "forge.ragdoll";

/// The type key of the transform, as the editor files it.
const ENTITY_CATEGORY: &str = "Entity";

/// The category of the rendering types.
const RENDERING_CATEGORY: &str = "Rendering";

/// The category of the physics types.
const PHYSICS_CATEGORY: &str = "Physics";

/// A type with a key, label and category and the other fields at their
/// defaults.
fn schema(
    key: &str,
    label: &str,
    category: &str,
    properties: Vec<PropertySchema>,
) -> ComponentSchema {
    ComponentSchema {
        key: key.to_owned(),
        label: label.to_owned(),
        category: category.to_owned(),
        properties,
        ..ComponentSchema::default()
    }
}

/// The transform: the entity's own placement.
fn transform_schema() -> ComponentSchema {
    ComponentSchema {
        removable: false,
        intrinsic: true,
        ..schema(
            TRANSFORM,
            "Transform",
            ENTITY_CATEGORY,
            vec![PropertySchema::new(
                "transform",
                crate::PropertyValue::Transform(fr_transform::Transform::IDENTITY),
            )],
        )
    }
}

/// The name: the entity's own label.
fn name_schema() -> ComponentSchema {
    ComponentSchema {
        removable: false,
        intrinsic: true,
        ..schema(
            NAME,
            "Name",
            ENTITY_CATEGORY,
            vec![PropertySchema::text("name", "Entity")],
        )
    }
}

/// The mesh renderer: a glTF model or a primitive, with an optional material.
fn mesh_renderer_schema() -> ComponentSchema {
    schema(
        MESH_RENDERER,
        "Mesh Renderer",
        RENDERING_CATEGORY,
        vec![
            PropertySchema::asset("model").labelled("Model"),
            PropertySchema::enumeration(
                "primitive",
                &[
                    ("none", "None"),
                    ("cube", "Cube"),
                    ("sphere", "Sphere"),
                    ("capsule", "Capsule"),
                ],
                "none",
            )
            .labelled("Primitive"),
            PropertySchema::float("radius", 0.5)
                .range(0.001, 1000.0)
                .labelled("Capsule radius"),
            PropertySchema::float("length", 1.0)
                .range(0.0, 1000.0)
                .labelled("Capsule length"),
            PropertySchema::boolean("override_material", false).labelled("Override material"),
            PropertySchema::color("base_color", Color::rgb(1.0, 1.0, 1.0)).labelled("Base color"),
            PropertySchema::float("metallic", 0.0)
                .range(0.0, 1.0)
                .labelled("Metallic"),
            PropertySchema::float("roughness", 0.5)
                .range(0.0, 1.0)
                .labelled("Roughness"),
            PropertySchema::color("emissive", Color::rgb(0.0, 0.0, 0.0)).labelled("Emissive"),
            PropertySchema::float("emissive_intensity", 1.0)
                .range(0.0, 1000.0)
                .labelled("Emissive intensity"),
            PropertySchema::boolean("double_sided", false).labelled("Double sided"),
        ],
    )
}

/// The camera: a viewpoint along the entity's forward axis.
fn camera_schema() -> ComponentSchema {
    schema(
        CAMERA,
        "Camera",
        "Camera",
        vec![
            PropertySchema::enumeration(
                "projection",
                &[
                    ("perspective", "Perspective"),
                    ("orthographic", "Orthographic"),
                ],
                "perspective",
            )
            .labelled("Projection"),
            PropertySchema::float("fov_y_degrees", 60.0)
                .range(1.0, 170.0)
                .labelled("Field of view"),
            PropertySchema::float("orthographic_height", 10.0)
                .range(0.01, 10000.0)
                .labelled("Orthographic height"),
            PropertySchema::float("near", 0.05)
                .range(0.001, 1000.0)
                .labelled("Near"),
            PropertySchema::float("far", 500.0)
                .range(0.01, 100_000.0)
                .labelled("Far"),
            PropertySchema::float("exposure", 1.0)
                .range(0.0, 100.0)
                .labelled("Exposure"),
            PropertySchema::boolean("current", true).labelled("Current"),
        ],
    )
}

/// The light: directional, point or spot, shining along the entity's forward
/// axis.
fn light_schema() -> ComponentSchema {
    schema(
        LIGHT,
        "Light",
        "Light",
        vec![
            PropertySchema::enumeration(
                "kind",
                &[
                    ("directional", "Directional"),
                    ("point", "Point"),
                    ("spot", "Spot"),
                ],
                "point",
            )
            .labelled("Kind"),
            PropertySchema::color("color", Color::rgb(1.0, 1.0, 1.0)).labelled("Color"),
            PropertySchema::float("intensity", 10.0)
                .range(0.0, 100_000.0)
                .labelled("Intensity"),
            PropertySchema::float("range", 10.0)
                .range(0.0, 100_000.0)
                .labelled("Range"),
            PropertySchema::float("inner_angle_degrees", 20.0)
                .range(0.0, 89.0)
                .labelled("Inner angle"),
            PropertySchema::float("outer_angle_degrees", 30.0)
                .range(0.0, 89.0)
                .labelled("Outer angle"),
            PropertySchema::boolean("cast_shadows", false).labelled("Cast shadows"),
            PropertySchema::float("shadow_distance", 60.0)
                .range(0.0, 100_000.0)
                .labelled("Shadow distance"),
        ],
    )
}

/// The rigid body: how an entity moves; mass comes from the colliders.
fn rigid_body_schema() -> ComponentSchema {
    ComponentSchema {
        conflicting_types: vec![RAGDOLL.to_owned()],
        ..schema(
            RIGID_BODY,
            "Rigid Body",
            PHYSICS_CATEGORY,
            vec![
                PropertySchema::enumeration(
                    "body_type",
                    &[
                        ("static", "Static"),
                        ("kinematic", "Kinematic"),
                        ("dynamic", "Dynamic"),
                    ],
                    "dynamic",
                )
                .labelled("Body type"),
                PropertySchema::float("linear_damping", 0.0)
                    .range(0.0, 100.0)
                    .labelled("Linear damping"),
                PropertySchema::float("angular_damping", 0.0)
                    .range(0.0, 100.0)
                    .labelled("Angular damping"),
                PropertySchema::float("gravity_scale", 1.0)
                    .range(-100.0, 100.0)
                    .labelled("Gravity scale"),
            ],
        )
    }
}

/// The collider: a shape attached to the rigid body of the same entity.
fn collider_schema() -> ComponentSchema {
    ComponentSchema {
        allow_multiple: true,
        required_types: vec![RIGID_BODY.to_owned()],
        ..schema(
            COLLIDER,
            "Collider",
            PHYSICS_CATEGORY,
            vec![
                PropertySchema::enumeration(
                    "shape",
                    &[("sphere", "Sphere"), ("capsule", "Capsule"), ("box", "Box")],
                    "box",
                )
                .labelled("Shape"),
                PropertySchema::float("radius", 0.5)
                    .range(0.001, 1000.0)
                    .labelled("Radius"),
                PropertySchema::float("half_height", 0.5)
                    .range(0.0, 1000.0)
                    .labelled("Half height"),
                PropertySchema::vec3("half_extents", Vec3::splat(0.5)).labelled("Half extents"),
                PropertySchema::vec3("offset", Vec3::ZERO).labelled("Offset"),
                PropertySchema::float("density", 1000.0)
                    .range(0.0, 1_000_000.0)
                    .labelled("Density"),
                PropertySchema::float("friction", 0.6)
                    .range(0.0, 100.0)
                    .labelled("Friction"),
                PropertySchema::float("restitution", 0.0)
                    .range(0.0, 1.0)
                    .labelled("Restitution"),
                PropertySchema::float("rolling_resistance", 0.0)
                    .range(0.0, 1.0)
                    .labelled("Rolling resistance"),
                PropertySchema::boolean("sensor", false).labelled("Sensor"),
            ],
        )
    }
}

/// The ragdoll: a humanoid of capsule and box bodies joined by limited joints,
/// built at the entity's placement.
fn ragdoll_schema() -> ComponentSchema {
    ComponentSchema {
        conflicting_types: vec![RIGID_BODY.to_owned()],
        ..schema(
            RAGDOLL,
            "Ragdoll",
            PHYSICS_CATEGORY,
            vec![
                PropertySchema::float("friction_torque", 5.0)
                    .range(0.0, 1000.0)
                    .labelled("Friction torque"),
                PropertySchema::float("joint_hertz", 1.0)
                    .range(0.0, 1000.0)
                    .labelled("Joint stiffness"),
                PropertySchema::float("joint_damping_ratio", 0.7)
                    .range(0.0, 100.0)
                    .labelled("Joint damping"),
                PropertySchema::float("density", 1000.0)
                    .range(0.0, 1_000_000.0)
                    .labelled("Density"),
                PropertySchema::color("color", Color::rgb(0.95, 0.3, 0.25)).labelled("Color"),
                PropertySchema::float("roughness", 0.6)
                    .range(0.0, 1.0)
                    .labelled("Roughness"),
                PropertySchema::boolean("show_bones", true).labelled("Show bones"),
                PropertySchema::color("bone_color", Color::rgb(0.05, 0.05, 0.06))
                    .labelled("Bone color"),
            ],
        )
    }
}

/// The built-in types in registration order, which is also the order the stage
/// realizes an entity's components in.
pub fn builtin_schemas() -> SchemaSet {
    let mut set = SchemaSet::empty();
    for schema in [
        transform_schema(),
        name_schema(),
        camera_schema(),
        light_schema(),
        mesh_renderer_schema(),
        rigid_body_schema(),
        collider_schema(),
        ragdoll_schema(),
    ] {
        set.add(schema);
    }
    set
}
