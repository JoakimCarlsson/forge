//! The contribution for `forge.light`.

use std::rc::Rc;

use fr_authoring::ComponentInit;
use fr_color::Rgba;
use fr_document::PropertyValue;
use fr_document::builtin_schema::LIGHT;
use fr_math::{Aabb, Vec3};
use fr_transform::Transform;
use fr_ui::IconName;

use crate::subsystems::{
    BoundsContext, CreateEntry, GizmoSink, Properties, Subsystem, SubsystemRegistry,
    quat_from_degrees,
};

/// The group of the light entries in the Create menu.
const LIGHT_CATEGORY: &str = "Light";

/// Half the size of the box a light can be picked by.
const PICK_HALF_SIZE: f32 = 0.25;

/// The length of the arrow drawn for a directional light.
const DIRECTIONAL_ARROW_LENGTH: f32 = 1.5;

/// The radius of the disc a directional light's rays leave.
const DIRECTIONAL_DISC_RADIUS: f32 = 0.3;

/// The colour of a light's wire shape.
const LIGHT_COLOR: Rgba = Rgba::hex(0xffd966);

/// The pitch a new directional light is created with, in degrees.
const DIRECTIONAL_PITCH: f32 = -50.0;

/// The yaw a new directional light is created with, in degrees.
const DIRECTIONAL_YAW: f32 = -30.0;

/// How far above its creation point a new point or spot light is placed.
const LAMP_HEIGHT: f32 = 3.0;

/// Adds the light subsystem to the registry.
pub fn register(registry: &mut SubsystemRegistry) {
    registry.add(Rc::new(LightSubsystem));
}

/// Contributes the light.
struct LightSubsystem;

impl LightSubsystem {
    /// The light of one kind as a creation spec with its brightness.
    fn component(kind: &str, intensity: f32, range: f32, shadows: bool) -> ComponentInit {
        ComponentInit::new(LIGHT)
            .with("kind", PropertyValue::Enum(kind.to_owned()))
            .with("intensity", PropertyValue::Float(intensity))
            .with("range", PropertyValue::Float(range))
            .with("cast_shadows", PropertyValue::Bool(shadows))
    }
}

impl Subsystem for LightSubsystem {
    /// The light's type key.
    fn kind(&self) -> &'static str {
        LIGHT
    }

    /// The light bulb icon.
    fn icon(&self) -> IconName {
        IconName::Light
    }

    /// A small box around the light.
    fn bounds(
        &self,
        _component: &Properties<'_>,
        _context: &mut BoundsContext<'_>,
    ) -> Option<Aabb> {
        Some(Aabb::from_center_extents(
            Vec3::ZERO,
            Vec3::splat(PICK_HALF_SIZE),
        ))
    }

    /// The range of a point light as three circles, the cone of a spot light
    /// with its inner cone, and the arrow and ray disc of a directional light.
    fn gizmo(&self, component: &Properties<'_>, world: &Transform, sink: &mut GizmoSink) {
        let placement = Transform {
            scale: Vec3::ONE,
            ..*world
        };
        sink.set_placement(placement);
        sink.set_color(LIGHT_COLOR);
        let range = component.float("range");
        match component.text("kind") {
            "directional" => {
                sink.arrow(
                    Vec3::ZERO,
                    Vec3::NEG_Z * DIRECTIONAL_ARROW_LENGTH,
                    DIRECTIONAL_ARROW_LENGTH * 0.15,
                );
                sink.circle(Vec3::ZERO, Vec3::Z, DIRECTIONAL_DISC_RADIUS);
                for side in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y] {
                    let start = side * DIRECTIONAL_DISC_RADIUS;
                    sink.line(start, start + Vec3::NEG_Z * DIRECTIONAL_ARROW_LENGTH);
                }
            }
            "spot" => {
                let outer = component.float("outer_angle_degrees").to_radians();
                let inner = component.float("inner_angle_degrees").to_radians();
                sink.wire_cone(Vec3::ZERO, Vec3::NEG_Z, range, outer);
                let inner_center = Vec3::NEG_Z * range;
                sink.circle(inner_center, Vec3::Z, range * inner.tan());
            }
            _ => sink.wire_sphere(Vec3::ZERO, range),
        }
    }

    /// Directional, Point and Spot light.
    fn create_entries(&self) -> Vec<CreateEntry> {
        let sun = Transform::from_rotation(quat_from_degrees(Vec3::new(
            DIRECTIONAL_PITCH,
            DIRECTIONAL_YAW,
            0.0,
        )));
        let lamp = Transform::from_translation(Vec3::Y * LAMP_HEIGHT);
        let downward = lamp.with_rotation(quat_from_degrees(Vec3::new(-90.0, 0.0, 0.0)));
        vec![
            CreateEntry::new(
                "Directional Light",
                LIGHT_CATEGORY,
                IconName::Sun,
                "Directional Light",
            )
            .at(sun)
            .with_component(Self::component("directional", 3.0, 0.0, true)),
            CreateEntry::new(
                "Point Light",
                LIGHT_CATEGORY,
                IconName::Light,
                "Point Light",
            )
            .at(lamp)
            .with_component(Self::component("point", 40.0, 15.0, false)),
            CreateEntry::new("Spot Light", LIGHT_CATEGORY, IconName::Light, "Spot Light")
                .at(downward)
                .with_component(Self::component("spot", 90.0, 25.0, false)),
        ]
    }
}
