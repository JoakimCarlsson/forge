//! The 3D line segments a component draws in the viewport.

use fr_color::Rgba;
use fr_math::{Quat, Vec3};
use fr_transform::Transform;

/// The segments of a full circle.
const CIRCLE_SEGMENTS: usize = 32;

/// One 3D line segment of a gizmo, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoLine {
    /// The start of the segment.
    pub from: Vec3,
    /// The end of the segment.
    pub to: Vec3,
    /// The colour it is drawn in.
    pub color: Rgba,
}

/// Collects the segments a contribution draws. Shapes are described in the
/// local space of the sink's placement, which a contribution sets to the
/// component's world transform, and are stored in world space for the viewport
/// to project.
#[derive(Clone, Debug)]
pub struct GizmoSink {
    /// The segments collected so far.
    lines: Vec<GizmoLine>,
    /// The colour new segments take.
    color: Rgba,
    /// The placement of the local space in the world.
    placement: Transform,
}

impl GizmoSink {
    /// An empty sink whose segments take `color` until it is changed.
    pub fn new(color: Rgba) -> Self {
        Self {
            lines: Vec::new(),
            color,
            placement: Transform::IDENTITY,
        }
    }

    /// The segments collected, in world space.
    pub fn lines(&self) -> &[GizmoLine] {
        &self.lines
    }

    /// Takes the segments out of the sink.
    pub fn into_lines(self) -> Vec<GizmoLine> {
        self.lines
    }

    /// The colour new segments take.
    pub fn color(&self) -> Rgba {
        self.color
    }

    /// Sets the colour new segments take.
    pub fn set_color(&mut self, color: Rgba) {
        self.color = color;
    }

    /// Sets the placement of the local space shapes are described in.
    pub fn set_placement(&mut self, placement: Transform) {
        self.placement = placement;
    }

    /// Moves a local point into the world.
    fn world(&self, local: Vec3) -> Vec3 {
        self.placement.matrix().transform_point3(local)
    }

    /// Adds one segment between two local points.
    pub fn line(&mut self, from: Vec3, to: Vec3) {
        self.lines.push(GizmoLine {
            from: self.world(from),
            to: self.world(to),
            color: self.color,
        });
    }

    /// Adds the segments joining consecutive local points, and the last to the
    /// first when `closed`.
    pub fn polyline(&mut self, points: &[Vec3], closed: bool) {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1]);
        }
        if let (true, Some(first), Some(last)) = (closed, points.first(), points.last()) {
            self.line(*last, *first);
        }
    }

    /// Adds a circle of `radius` around `center` in the plane whose normal is
    /// `normal`.
    pub fn circle(&mut self, center: Vec3, normal: Vec3, radius: f32) {
        let (u, v) = plane_basis(normal);
        let points: Vec<Vec3> = (0..CIRCLE_SEGMENTS)
            .map(|step| {
                let angle = step as f32 / CIRCLE_SEGMENTS as f32 * std::f32::consts::TAU;
                center + (u * angle.cos() + v * angle.sin()) * radius
            })
            .collect();
        self.polyline(&points, true);
    }

    /// Adds the twelve edges of a box of half sizes `half` around `center`.
    pub fn wire_box(&mut self, center: Vec3, half: Vec3) {
        let corner = |index: usize| {
            center
                + Vec3::new(
                    if index & 1 == 0 { -half.x } else { half.x },
                    if index & 2 == 0 { -half.y } else { half.y },
                    if index & 4 == 0 { -half.z } else { half.z },
                )
        };
        for index in 0..8 {
            for bit in [1, 2, 4] {
                if index & bit == 0 {
                    self.line(corner(index), corner(index | bit));
                }
            }
        }
    }

    /// Adds a sphere as three circles, one around each axis.
    pub fn wire_sphere(&mut self, center: Vec3, radius: f32) {
        for normal in [Vec3::X, Vec3::Y, Vec3::Z] {
            self.circle(center, normal, radius);
        }
    }

    /// Adds a capsule along the Y axis: two rings, the silhouette lines and a
    /// half circle over each cap, as the cylinder and the caps of the collider.
    pub fn wire_capsule(&mut self, center: Vec3, radius: f32, half_height: f32) {
        let top = center + Vec3::Y * half_height;
        let bottom = center - Vec3::Y * half_height;
        self.circle(top, Vec3::Y, radius);
        self.circle(bottom, Vec3::Y, radius);
        for side in [Vec3::X, Vec3::Z] {
            self.line(top + side * radius, bottom + side * radius);
            self.line(top - side * radius, bottom - side * radius);
            self.arc(top, side, Vec3::Y, radius, 1.0);
            self.arc(bottom, side, Vec3::Y, radius, -1.0);
        }
    }

    /// Adds a half circle around `center` from `-from` to `from`, bulging
    /// toward `up` scaled by `sign`.
    fn arc(&mut self, center: Vec3, from: Vec3, up: Vec3, radius: f32, sign: f32) {
        let steps = CIRCLE_SEGMENTS / 2;
        let points: Vec<Vec3> = (0..=steps)
            .map(|step| {
                let angle = step as f32 / steps as f32 * std::f32::consts::PI;
                center + (from * angle.cos() + up * sign * angle.sin()) * radius
            })
            .collect();
        self.polyline(&points, false);
    }

    /// Adds a cone from `apex` along `direction` for `length`, opening by
    /// `half_angle` radians: the rim circle and four edges.
    pub fn wire_cone(&mut self, apex: Vec3, direction: Vec3, length: f32, half_angle: f32) {
        let axis = direction.normalize_or_zero();
        let radius = length * half_angle.tan();
        let center = apex + axis * length;
        self.circle(center, axis, radius);
        let (u, v) = plane_basis(axis);
        for side in [u, -u, v, -v] {
            self.line(apex, center + side * radius);
        }
    }

    /// Adds an arrow from `from` to `to` with a head of length `head`.
    pub fn arrow(&mut self, from: Vec3, to: Vec3, head: f32) {
        self.line(from, to);
        let axis = (to - from).normalize_or_zero();
        let (u, v) = plane_basis(axis);
        for side in [u, -u, v, -v] {
            self.line(to, to - axis * head + side * head * 0.4);
        }
    }
}

/// Two unit vectors that, with `normal`, form a right handed basis.
pub fn plane_basis(normal: Vec3) -> (Vec3, Vec3) {
    let axis = normal.try_normalize().unwrap_or(Vec3::Z);
    let rotation = Quat::from_rotation_arc(Vec3::Z, axis);
    (rotation * Vec3::X, rotation * Vec3::Y)
}
