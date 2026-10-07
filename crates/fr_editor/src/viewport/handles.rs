//! The geometry of the transform gizmo's handles: where each one is in the
//! world, which one the pointer is over, and how a drag along it maps to a
//! change.

use fr_math::{Point, Quat, Ray3d, Vec3};

use super::projector::{Projector, distance_to_segment};

/// The size of the gizmo on screen, in logical pixels.
pub const GIZMO_PIXELS: f32 = 96.0;

/// How near the pointer must be to a line handle to hover it, in logical
/// pixels.
const PICK_RADIUS: f32 = 9.0;

/// The size of the uniform scale handle at the pivot, as a radius in logical
/// pixels.
const UNIFORM_RADIUS: f32 = 11.0;

/// Where the corner of a plane handle nearest the pivot lies, as a fraction of
/// the gizmo's length.
const PLANE_NEAR: f32 = 0.3;

/// Where the far corner of a plane handle lies, as a fraction of the length.
const PLANE_FAR: f32 = 0.6;

/// The radius of a rotation ring, as a fraction of the length.
const RING_RADIUS: f32 = 0.85;

/// The length of a scale handle, as a fraction of the length.
const SCALE_LENGTH: f32 = 0.9;

/// How many segments a rotation ring is made of.
const RING_SEGMENTS: usize = 48;

/// The smallest ratio of a pointer's distance from the pivot that a uniform
/// scale drag compares against, which keeps it from dividing by zero.
const MINIMUM_PIVOT_DISTANCE: f32 = 4.0;

/// The least factor a scale drag may produce.
const MINIMUM_SCALE_FACTOR: f32 = 0.01;

/// One of the three axes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    /// The X axis.
    X,
    /// The Y axis.
    Y,
    /// The Z axis.
    Z,
}

impl Axis {
    /// The three axes in order.
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    /// The unit vector of the axis.
    pub fn unit(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }

    /// The position of the axis among the components of a vector.
    pub fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    /// The two other axes, in an order that, with this one, makes a right
    /// handed basis.
    pub fn others(self) -> (Axis, Axis) {
        match self {
            Self::X => (Axis::Y, Axis::Z),
            Self::Y => (Axis::Z, Axis::X),
            Self::Z => (Axis::X, Axis::Y),
        }
    }
}

/// A part of the gizmo the pointer can take hold of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Handle {
    /// An arrow or scale handle along an axis.
    Axis(Axis),
    /// A square in the plane perpendicular to an axis.
    Plane(Axis),
    /// A ring around an axis.
    Ring(Axis),
    /// The box at the pivot that scales every axis together.
    Uniform,
}

/// What a tool shows: its handles in the order they are drawn and picked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleSet {
    /// Arrows and plane squares.
    Move,
    /// Three rings.
    Rotate,
    /// Three scale handles and the uniform box.
    Scale,
}

impl HandleSet {
    /// The handles of the set.
    pub fn handles(self) -> Vec<Handle> {
        match self {
            Self::Move => Axis::ALL
                .iter()
                .map(|axis| Handle::Plane(*axis))
                .chain(Axis::ALL.iter().map(|axis| Handle::Axis(*axis)))
                .collect(),
            Self::Rotate => Axis::ALL.iter().map(|axis| Handle::Ring(*axis)).collect(),
            Self::Scale => Axis::ALL
                .iter()
                .map(|axis| Handle::Axis(*axis))
                .chain([Handle::Uniform])
                .collect(),
        }
    }
}

/// Where the gizmo is: its pivot, its orientation and its length in the world
/// at the size it has on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoFrame {
    /// The point the handles are centred on.
    pub pivot: Vec3,
    /// The orientation of the handles' axes.
    pub basis: Quat,
    /// The length of an axis handle in the world.
    pub length: f32,
}

impl GizmoFrame {
    /// The direction of an axis in the world.
    pub fn direction(&self, axis: Axis) -> Vec3 {
        self.basis * axis.unit()
    }
}

/// A 3D segment of a handle; `front` is false for the half of a ring that
/// faces away from the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandleSegment {
    /// The start.
    pub from: Vec3,
    /// The end.
    pub to: Vec3,
    /// Whether the segment faces the camera.
    pub front: bool,
}

/// The segments a handle is drawn and picked by, for the handle set that shows
/// it.
pub fn handle_segments(
    set: HandleSet,
    handle: Handle,
    frame: &GizmoFrame,
    camera_position: Vec3,
) -> Vec<HandleSegment> {
    let solid = |from, to| HandleSegment {
        from,
        to,
        front: true,
    };
    match handle {
        Handle::Axis(axis) => {
            let length = if set == HandleSet::Scale {
                SCALE_LENGTH
            } else {
                1.0
            };
            vec![solid(
                frame.pivot,
                frame.pivot + frame.direction(axis) * frame.length * length,
            )]
        }
        Handle::Plane(axis) => plane_corners(frame, axis)
            .windows(2)
            .map(|pair| solid(pair[0], pair[1]))
            .collect(),
        Handle::Ring(axis) => ring_segments(frame, axis, camera_position),
        Handle::Uniform => Vec::new(),
    }
}

/// The corners of a plane handle, closed by repeating the first.
pub fn plane_corners(frame: &GizmoFrame, axis: Axis) -> [Vec3; 5] {
    let (first, second) = axis.others();
    let (a, b) = (frame.direction(first), frame.direction(second));
    let at = |u: f32, v: f32| frame.pivot + (a * u + b * v) * frame.length;
    let corners = [
        at(PLANE_NEAR, PLANE_NEAR),
        at(PLANE_FAR, PLANE_NEAR),
        at(PLANE_FAR, PLANE_FAR),
        at(PLANE_NEAR, PLANE_FAR),
    ];
    [corners[0], corners[1], corners[2], corners[3], corners[0]]
}

/// The segments of a rotation ring, each marked by whether it faces the camera.
fn ring_segments(frame: &GizmoFrame, axis: Axis, camera_position: Vec3) -> Vec<HandleSegment> {
    let (first, second) = axis.others();
    let (u, v) = (frame.direction(first), frame.direction(second));
    let radius = frame.length * RING_RADIUS;
    let point = |step: usize| {
        let angle = step as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
        frame.pivot + (u * angle.cos() + v * angle.sin()) * radius
    };
    let to_camera = camera_position - frame.pivot;
    (0..RING_SEGMENTS)
        .map(|step| {
            let (from, to) = (point(step), point(step + 1));
            let middle = (from + to) * 0.5 - frame.pivot;
            HandleSegment {
                from,
                to,
                front: middle.dot(to_camera) > 0.0,
            }
        })
        .collect()
}

/// Whether a window point lies inside a convex quad given by its projected
/// corners.
fn inside_quad(point: Point, corners: &[Point]) -> bool {
    let mut sign = 0.0_f32;
    for index in 0..corners.len() {
        let (a, b) = (corners[index], corners[(index + 1) % corners.len()]);
        let cross = (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x);
        if cross.abs() < 1e-6 {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if sign != cross.signum() {
            return false;
        }
    }
    true
}

/// The handle of a set under a window point: a plane square it is inside, the
/// uniform box, otherwise the line or ring nearest to it within the pick
/// radius.
pub fn hit_test(
    set: HandleSet,
    frame: &GizmoFrame,
    projector: &Projector,
    pointer: Point,
) -> Option<Handle> {
    let camera_position = projector.camera().position;
    let mut best: Option<(Handle, f32)> = None;
    for handle in set.handles() {
        let score = match handle {
            Handle::Plane(axis) => {
                let corners: Option<Vec<Point>> = plane_corners(frame, axis)[..4]
                    .iter()
                    .map(|corner| projector.project(*corner))
                    .collect();
                corners
                    .filter(|corners| inside_quad(pointer, corners))
                    .map(|_| 0.0)
            }
            Handle::Uniform => projector.project(frame.pivot).and_then(|center| {
                let distance = (pointer.x - center.x).hypot(pointer.y - center.y);
                (distance <= UNIFORM_RADIUS).then_some(0.0)
            }),
            _ => nearest_segment(
                handle_segments(set, handle, frame, camera_position),
                projector,
                pointer,
            ),
        };
        if let Some(score) = score
            && best.is_none_or(|(_, current)| score < current)
        {
            best = Some((handle, score));
        }
    }
    best.map(|(handle, _)| handle)
}

/// The distance in pixels from a point to the nearest front facing segment,
/// when it is within the pick radius.
fn nearest_segment(
    segments: Vec<HandleSegment>,
    projector: &Projector,
    pointer: Point,
) -> Option<f32> {
    segments
        .iter()
        .filter(|segment| segment.front)
        .filter_map(|segment| projector.project_segment(segment.from, segment.to))
        .map(|(from, to)| distance_to_segment(pointer, from, to))
        .filter(|distance| *distance <= PICK_RADIUS)
        .reduce(f32::min)
}

/// What a drag started on a handle measures against: the parameter along an
/// axis, the point in a plane, the angle around an axis, or the distance from
/// the pivot on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// A distance along the handle's axis from the pivot.
    Along(f32),
    /// A point of the handle's plane.
    Point(Vec3),
    /// An angle around the handle's axis, in radians.
    Angle(f32),
    /// The pointer's distance from the pivot on screen, in logical pixels.
    Screen(f32),
}

/// A change a drag produces, in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Delta {
    /// Moves by a vector.
    Translate(Vec3),
    /// Rotates around the pivot.
    Rotate(Quat),
    /// Multiplies each axis of the scale.
    Scale(Vec3),
}

/// How far a drag is rounded when snapping is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    /// The step of a move, in world units.
    pub translate: f32,
    /// The step of a rotation, in radians.
    pub rotate: f32,
    /// The step of a scale factor.
    pub scale: f32,
}

impl Default for Snap {
    /// Half a unit, fifteen degrees and a tenth.
    fn default() -> Self {
        Self {
            translate: 0.5,
            rotate: 15.0_f32.to_radians(),
            scale: 0.1,
        }
    }
}

/// Rounds a value to a multiple of a step.
fn snapped(value: f32, step: Option<f32>) -> f32 {
    match step {
        Some(step) if step > 0.0 => (value / step).round() * step,
        _ => value,
    }
}

/// The distance along a line through `origin` along the unit `axis` at which it
/// passes closest to a ray; none when the two are parallel.
pub fn along_axis(origin: Vec3, axis: Vec3, ray: &Ray3d) -> Option<f32> {
    let direction = ray.direction.as_vec3();
    let between = origin - ray.origin;
    let alignment = axis.dot(direction);
    let denominator = 1.0 - alignment * alignment;
    if denominator.abs() < 1e-5 {
        return None;
    }
    Some((alignment * direction.dot(between) - axis.dot(between)) / denominator)
}

/// The point where a ray meets the plane through `pivot` with the given
/// normal.
fn plane_point(pivot: Vec3, normal: Vec3, ray: &Ray3d) -> Option<Vec3> {
    let plane = fr_math::Plane::from_normal_and_point(normal, pivot);
    ray.intersect_plane(&plane)
        .map(|distance| ray.point_at(distance))
}

/// The angle of a point around the pivot in the plane of a ring.
fn ring_angle(frame: &GizmoFrame, axis: Axis, point: Vec3) -> f32 {
    let (first, second) = axis.others();
    let offset = point - frame.pivot;
    offset
        .dot(frame.direction(second))
        .atan2(offset.dot(frame.direction(first)))
}

/// The anchor of a drag that starts at a pointer position on a handle.
pub fn anchor_for(
    handle: Handle,
    frame: &GizmoFrame,
    projector: &Projector,
    pointer: Point,
) -> Option<Anchor> {
    let ray = projector.ray(pointer)?;
    match handle {
        Handle::Axis(axis) => {
            along_axis(frame.pivot, frame.direction(axis), &ray).map(Anchor::Along)
        }
        Handle::Plane(axis) => {
            plane_point(frame.pivot, frame.direction(axis), &ray).map(Anchor::Point)
        }
        Handle::Ring(axis) => plane_point(frame.pivot, frame.direction(axis), &ray)
            .map(|point| Anchor::Angle(ring_angle(frame, axis, point))),
        Handle::Uniform => {
            let center = projector.project(frame.pivot)?;
            let distance = (pointer.x - center.x).hypot(pointer.y - center.y);
            Some(Anchor::Screen(distance.max(MINIMUM_PIVOT_DISTANCE)))
        }
    }
}

/// The change a drag has produced once the pointer is at a new position; none
/// while the pointer's ray cannot reach what the handle measures.
pub fn solve_drag(
    handle: Handle,
    set: HandleSet,
    anchor: Anchor,
    frame: &GizmoFrame,
    projector: &Projector,
    pointer: Point,
    snap: Option<Snap>,
) -> Option<Delta> {
    let ray = projector.ray(pointer)?;
    match (handle, anchor) {
        (Handle::Axis(axis), Anchor::Along(start)) => {
            let direction = frame.direction(axis);
            let now = along_axis(frame.pivot, direction, &ray)?;
            if set == HandleSet::Scale {
                let factor = 1.0 + (now - start) / frame.length.max(1e-4);
                let factor = snapped(factor, snap.map(|snap| snap.scale));
                let mut scale = Vec3::ONE;
                scale[axis.index()] = factor.max(MINIMUM_SCALE_FACTOR);
                Some(Delta::Scale(scale))
            } else {
                let distance = snapped(now - start, snap.map(|snap| snap.translate));
                Some(Delta::Translate(direction * distance))
            }
        }
        (Handle::Plane(axis), Anchor::Point(start)) => {
            let now = plane_point(frame.pivot, frame.direction(axis), &ray)?;
            let (first, second) = axis.others();
            let (a, b) = (frame.direction(first), frame.direction(second));
            let step = snap.map(|snap| snap.translate);
            let moved = now - start;
            Some(Delta::Translate(
                a * snapped(moved.dot(a), step) + b * snapped(moved.dot(b), step),
            ))
        }
        (Handle::Ring(axis), Anchor::Angle(start)) => {
            let point = plane_point(frame.pivot, frame.direction(axis), &ray)?;
            let angle = snapped(
                ring_angle(frame, axis, point) - start,
                snap.map(|snap| snap.rotate),
            );
            Some(Delta::Rotate(Quat::from_axis_angle(
                frame.direction(axis),
                angle,
            )))
        }
        (Handle::Uniform, Anchor::Screen(start)) => {
            let center = projector.project(frame.pivot)?;
            let distance = (pointer.x - center.x).hypot(pointer.y - center.y);
            let factor = snapped(distance / start, snap.map(|snap| snap.scale));
            Some(Delta::Scale(Vec3::splat(factor.max(MINIMUM_SCALE_FACTOR))))
        }
        _ => None,
    }
}
