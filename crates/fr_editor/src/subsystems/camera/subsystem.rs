//! The contribution for `forge.camera`.

use std::rc::Rc;

use fr_authoring::ComponentInit;
use fr_color::Rgba;
use fr_document::builtin_schema::CAMERA;
use fr_math::{Aabb, Vec3};
use fr_transform::Transform;
use fr_ui::IconName;

use crate::subsystems::{
    BoundsContext, CreateEntry, GizmoSink, Properties, Subsystem, SubsystemRegistry,
};

/// The group of the camera entry in the Create menu.
const CAMERA_CATEGORY: &str = "Camera";

/// Half the size of the box a camera can be picked by.
const PICK_HALF_SIZE: f32 = 0.25;

/// The length of the frustum drawn from the camera, in world units, or its
/// far plane when that is nearer.
const FRUSTUM_DEPTH: f32 = 1.5;

/// The aspect ratio the frustum is drawn at.
const FRUSTUM_ASPECT: f32 = 16.0 / 9.0;

/// The colour of the camera's wire shape.
const CAMERA_COLOR: Rgba = Rgba::hex(0xd9d9d9);

/// Adds the camera subsystem to the registry.
pub fn register(registry: &mut SubsystemRegistry) {
    registry.add(Rc::new(CameraSubsystem));
}

/// Contributes the camera.
struct CameraSubsystem;

impl Subsystem for CameraSubsystem {
    /// The camera's type key.
    fn kind(&self) -> &'static str {
        CAMERA
    }

    /// The camera icon.
    fn icon(&self) -> IconName {
        IconName::Camera
    }

    /// A small box around the camera.
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

    /// The frustum as a pyramid from the camera along its forward axis, with a
    /// triangle over the top edge to show which way is up.
    fn gizmo(&self, component: &Properties<'_>, world: &Transform, sink: &mut GizmoSink) {
        let placement = Transform {
            scale: Vec3::ONE,
            ..*world
        };
        sink.set_placement(placement);
        sink.set_color(CAMERA_COLOR);
        let depth = FRUSTUM_DEPTH.min(component.float("far"));
        let (half_width, half_height) = if component.text("projection") == "orthographic" {
            let half_height = component.float("orthographic_height") * 0.5;
            (half_height * FRUSTUM_ASPECT, half_height)
        } else {
            let half_height = depth * (component.float("fov_y_degrees").to_radians() * 0.5).tan();
            (half_height * FRUSTUM_ASPECT, half_height)
        };
        let corners = [
            Vec3::new(-half_width, -half_height, -depth),
            Vec3::new(half_width, -half_height, -depth),
            Vec3::new(half_width, half_height, -depth),
            Vec3::new(-half_width, half_height, -depth),
        ];
        sink.polyline(&corners, true);
        for corner in corners {
            sink.line(Vec3::ZERO, corner);
        }
        let top = half_height * 1.35;
        sink.polyline(
            &[
                Vec3::new(-half_width * 0.3, half_height, -depth),
                Vec3::new(0.0, top, -depth),
                Vec3::new(half_width * 0.3, half_height, -depth),
            ],
            false,
        );
    }

    /// The Camera entry.
    fn create_entries(&self) -> Vec<CreateEntry> {
        vec![
            CreateEntry::new("Camera", CAMERA_CATEGORY, IconName::Camera, "Camera")
                .with_component(ComponentInit::new(CAMERA)),
        ]
    }
}
