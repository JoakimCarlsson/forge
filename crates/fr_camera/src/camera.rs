//! A viewer looking from a position at a target.

use fr_math::{Dir3, Mat4, Ray3d, Vec2, Vec3, look_at};

use crate::frustum::Frustum;
use crate::projection::Projection;
use crate::viewport::Viewport;

/// A camera looking from a position at a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Where the camera is, in world space.
    pub position: Vec3,
    /// The point the camera looks at.
    pub target: Vec3,
    /// The direction that is up on screen.
    pub up: Vec3,
    /// How view space maps into clip space.
    pub projection: Projection,
    /// The multiplier applied to scene radiance before tonemapping.
    pub exposure: f32,
}

impl Camera {
    /// The matrix taking world space into the camera's view space.
    pub fn view(&self) -> Mat4 {
        look_at(self.position, self.target, self.up)
    }

    /// The matrix taking view space into clip space for a viewport of `aspect` width over height.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        self.projection.matrix(aspect)
    }

    /// The matrix taking world space into clip space for a viewport of `aspect` width over height.
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect) * self.view()
    }

    /// What the camera sees through a viewport of `aspect` width over height.
    pub fn frustum(&self, aspect: f32) -> Frustum {
        Frustum::from_view_projection(&self.view_projection(aspect))
    }

    /// The ray from the camera through `position` of `viewport`, measured from its top-left corner.
    ///
    /// None when the viewport is empty or the camera has no usable view.
    pub fn viewport_to_world(&self, viewport: Viewport, position: Vec2) -> Option<Ray3d> {
        if viewport.size.x <= 0.0 || viewport.size.y <= 0.0 {
            return None;
        }
        let ndc = Vec2::new(
            position.x / viewport.size.x * 2.0 - 1.0,
            1.0 - position.y / viewport.size.y * 2.0,
        );
        let inverse = self.view_projection(viewport.aspect()).inverse();
        let near = inverse.project_point3(ndc.extend(0.0));
        let far = inverse.project_point3(ndc.extend(1.0));
        Dir3::new(far - near).map(|direction| Ray3d::new(near, direction))
    }

    /// Where `world` lands in `viewport`, measured from its top-left corner.
    ///
    /// None when the point is behind the camera or beyond its clip planes.
    pub fn world_to_viewport(&self, viewport: Viewport, world: Vec3) -> Option<Vec2> {
        let clip = self.view_projection(viewport.aspect()) * world.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        if !(0.0..=1.0).contains(&ndc.z) {
            return None;
        }
        Some(Vec2::new(
            (ndc.x + 1.0) * 0.5 * viewport.size.x,
            (1.0 - ndc.y) * 0.5 * viewport.size.y,
        ))
    }
}

impl Default for Camera {
    /// A camera five units back on Z, looking at the origin with the default perspective.
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Projection::default(),
            exposure: 1.0,
        }
    }
}
