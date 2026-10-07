//! Rays and their intersection with planes, spheres, boxes and triangles.

use glam::Vec3;

use crate::aabb::Aabb;
use crate::direction::Dir3;
use crate::plane::Plane;

/// The smallest speed along a normal or an edge that still counts as not parallel.
const PARALLEL_EPSILON: f32 = 1e-8;

/// A half line from `origin` along a unit `direction`.
///
/// Every intersection test returns the distance along the ray at which it
/// first touches the shape, or none when it misses; a ray that starts inside a
/// solid shape reports distance zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray3d {
    /// Where the ray starts.
    pub origin: Vec3,
    /// The way the ray travels.
    pub direction: Dir3,
}

impl Ray3d {
    /// A ray from `origin` along `direction`.
    pub const fn new(origin: Vec3, direction: Dir3) -> Self {
        Self { origin, direction }
    }

    /// The point `distance` along the ray.
    pub fn point_at(&self, distance: f32) -> Vec3 {
        self.origin + self.direction.as_vec3() * distance
    }

    /// The distance at which the ray reaches `plane`, unless it is parallel to it or pointing away.
    pub fn intersect_plane(&self, plane: &Plane) -> Option<f32> {
        let speed = plane.normal.dot(self.direction.as_vec3());
        if speed.abs() <= PARALLEL_EPSILON {
            return None;
        }
        let distance = -plane.separation(self.origin) / speed;
        (distance >= 0.0).then_some(distance)
    }

    /// The distance at which the ray enters the sphere of `radius` around `center`.
    pub fn intersect_sphere(&self, center: Vec3, radius: f32) -> Option<f32> {
        let offset = self.origin - center;
        let along = offset.dot(self.direction.as_vec3());
        let outside = offset.length_squared() - radius * radius;
        if outside > 0.0 && along > 0.0 {
            return None;
        }
        let discriminant = along * along - outside;
        if discriminant < 0.0 {
            return None;
        }
        Some((-along - discriminant.sqrt()).max(0.0))
    }

    /// The distance at which the ray enters `aabb`.
    pub fn intersect_aabb(&self, aabb: &Aabb) -> Option<f32> {
        aabb.ray_entry(self.origin, self.direction.as_vec3(), f32::INFINITY)
    }

    /// The distance at which the ray crosses the triangle `a`, `b`, `c`, seen from either side.
    pub fn intersect_triangle(&self, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
        let direction = self.direction.as_vec3();
        let edge_1 = b - a;
        let edge_2 = c - a;
        let pivot = direction.cross(edge_2);
        let determinant = edge_1.dot(pivot);
        if determinant.abs() <= PARALLEL_EPSILON {
            return None;
        }
        let inverse = 1.0 / determinant;
        let from_a = self.origin - a;
        let u = from_a.dot(pivot) * inverse;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let across = from_a.cross(edge_1);
        let v = direction.dot(across) * inverse;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let distance = edge_2.dot(across) * inverse;
        (distance >= 0.0).then_some(distance)
    }
}
