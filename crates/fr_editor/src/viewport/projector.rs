//! Projecting world points into the viewport's window rectangle and drawing 3D
//! lines there.

use fr_camera::{Camera, Projection, Viewport};
use fr_color::Rgba;
use fr_math::{Mat4, Point, Ray3d, Rect, Vec2, Vec3};
use fr_render::DrawList;

/// The largest coordinate a projected point is kept to, so a point just in
/// front of the camera cannot produce a quad too large to draw.
const COORDINATE_LIMIT: f32 = 100_000.0;

/// The smallest clip space depth a point may have to count as in front of the
/// camera.
const MINIMUM_W: f32 = 1e-4;

/// A camera looking into a rectangle of the window: the one place world points
/// become logical pixels and pointer positions become rays.
#[derive(Clone, Copy, Debug)]
pub struct Projector {
    /// The camera.
    camera: Camera,
    /// The rectangle of the window the camera draws into.
    rect: Rect,
    /// The matrix taking world space into clip space for the rectangle.
    view_projection: Mat4,
    /// The matrix taking world space into the camera's view space.
    view: Mat4,
}

impl Projector {
    /// A projector for a camera drawing into a rectangle of the window.
    pub fn new(camera: Camera, rect: Rect) -> Self {
        let aspect = if rect.size.height > 0.0 {
            rect.size.width / rect.size.height
        } else {
            1.0
        };
        Self {
            camera,
            rect,
            view_projection: camera.view_projection(aspect),
            view: camera.view(),
        }
    }

    /// The camera.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The rectangle of the window.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// The viewport of the rectangle, for the camera's own ray and projection
    /// functions.
    fn viewport(&self) -> Viewport {
        Viewport::new(self.rect.size.width, self.rect.size.height)
    }

    /// The ray from the camera through a point of the window.
    pub fn ray(&self, pointer: Point) -> Option<Ray3d> {
        self.camera.viewport_to_world(
            self.viewport(),
            Vec2::new(pointer.x - self.rect.left(), pointer.y - self.rect.top()),
        )
    }

    /// Where a world point lands in the window; none when it is behind the
    /// camera. A point beyond the far plane still projects.
    pub fn project(&self, world: Vec3) -> Option<Point> {
        let clip = self.view_projection * world.extend(1.0);
        if clip.w <= MINIMUM_W {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        let x = (ndc.x + 1.0) * 0.5 * self.rect.size.width;
        let y = (1.0 - ndc.y) * 0.5 * self.rect.size.height;
        Some(Point::new(
            self.rect.left() + x.clamp(-COORDINATE_LIMIT, COORDINATE_LIMIT),
            self.rect.top() + y.clamp(-COORDINATE_LIMIT, COORDINATE_LIMIT),
        ))
    }

    /// The distance of a point in front of the camera along its view
    /// direction; negative behind it.
    pub fn depth(&self, world: Vec3) -> f32 {
        -self.view.transform_point3(world).z
    }

    /// How many world units one logical pixel covers at a point, which keeps a
    /// handle the same size on screen; none behind the camera.
    pub fn world_per_pixel(&self, world: Vec3) -> Option<f32> {
        let height = self.rect.size.height.max(1.0);
        match self.camera.projection {
            Projection::Perspective(perspective) => {
                let depth = self.depth(world);
                (depth > perspective.near * 0.5)
                    .then(|| 2.0 * depth * (perspective.fov_y * 0.5).tan() / height)
            }
            Projection::Orthographic(orthographic) => Some(orthographic.height / height),
        }
    }

    /// The part of a segment in front of the near plane.
    pub fn clip_segment(&self, from: Vec3, to: Vec3) -> Option<(Vec3, Vec3)> {
        let near = self.camera.projection.near();
        let limit = -near;
        let start = self.view.transform_point3(from).z;
        let end = self.view.transform_point3(to).z;
        match (start < limit, end < limit) {
            (false, false) => None,
            (true, true) => Some((from, to)),
            (true, false) => Some((from, from.lerp(to, (limit - start) / (end - start)))),
            (false, true) => Some((from.lerp(to, (limit - start) / (end - start)), to)),
        }
    }

    /// The window positions of a segment's ends after clipping; none when it is
    /// out of view.
    pub fn project_segment(&self, from: Vec3, to: Vec3) -> Option<(Point, Point)> {
        let (from, to) = self.clip_segment(from, to)?;
        Some((self.project(from)?, self.project(to)?))
    }

    /// Draws a world segment as a line of `width` logical pixels.
    pub fn draw_line(&self, list: &mut DrawList, from: Vec3, to: Vec3, width: f32, color: Rgba) {
        if let Some((start, end)) = self.project_segment(from, to) {
            list.line(start, end, width, color);
        }
    }
}

/// The distance from a point to a segment, in the units of the points.
pub fn distance_to_segment(point: Point, start: Point, end: Point) -> f32 {
    let along = Vec2::new(end.x - start.x, end.y - start.y);
    let offset = Vec2::new(point.x - start.x, point.y - start.y);
    let length_squared = along.length_squared();
    let fraction = if length_squared > 1e-6 {
        (offset.dot(along) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (offset - along * fraction).length()
}
