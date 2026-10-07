//! Spatial queries: ray casts and bounds overlaps.

use fr_math::{Ray3d, Vec3};

use crate::body::BodyId;
use crate::math::normalize;
use crate::shape::ShapeId;
use crate::world::World;
use fr_math::Aabb;

/// Which shapes a query considers: a shape is considered when the query mask accepts its
/// category and its mask accepts the query category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryFilter {
    /// The category bits of the query.
    pub category: u64,
    /// The categories the query considers.
    pub mask: u64,
}

impl Default for QueryFilter {
    /// A query that considers everything.
    fn default() -> Self {
        Self {
            category: u64::MAX,
            mask: u64::MAX,
        }
    }
}

/// The place where a ray hit a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// The shape that was hit.
    pub shape: ShapeId,
    /// The body of the shape.
    pub body: BodyId,
    /// The hit point in the world.
    pub point: Vec3,
    /// The surface normal in the world.
    pub normal: Vec3,
    /// The fraction of the ray translation at the hit.
    pub fraction: f32,
}

impl World {
    /// Casts `ray` up to `max_distance` and calls `callback` with each hit, whose fraction is the
    /// distance over `max_distance`. The callback returns the fraction to continue with: the hit
    /// fraction to keep only closer hits, one to see every hit, or zero to stop.
    pub fn cast_ray<F>(&self, ray: &Ray3d, max_distance: f32, filter: &QueryFilter, callback: F)
    where
        F: FnMut(&RayHit) -> f32,
    {
        self.cast_segment(
            ray.origin,
            ray.direction.as_vec3() * max_distance,
            filter,
            callback,
        );
    }

    /// The closest hit of `ray` within `max_distance`.
    pub fn cast_ray_closest(
        &self,
        ray: &Ray3d,
        max_distance: f32,
        filter: &QueryFilter,
    ) -> Option<RayHit> {
        self.cast_segment_closest(ray.origin, ray.direction.as_vec3() * max_distance, filter)
    }

    /// Casts a segment from `origin` along `translation` and calls `callback` with each hit.
    /// The callback returns the fraction to continue with: the hit fraction to keep only closer
    /// hits, one to see every hit, or zero to stop.
    fn cast_segment<F>(
        &self,
        origin: Vec3,
        translation: Vec3,
        filter: &QueryFilter,
        mut callback: F,
    ) where
        F: FnMut(&RayHit) -> f32,
    {
        self.broad_phase
            .ray_cast(origin, translation, 1.0, |slot, max_fraction| {
                let Some(shape_id) = self.shapes.handle_at(slot as usize) else {
                    return max_fraction;
                };
                let Some(shape) = self.shapes.get(shape_id) else {
                    return max_fraction;
                };
                let accepted = (shape.filter.category & filter.mask) != 0
                    && (shape.filter.mask & filter.category) != 0;
                if shape.is_sensor || !accepted {
                    return max_fraction;
                }
                let Some(body) = self.bodies.get(shape.body) else {
                    return max_fraction;
                };
                let local_origin = body.pose.inv_transform_point(origin);
                let local_translation = body.pose.inv_rotate(translation);
                let Some(hit) =
                    shape
                        .geometry
                        .ray_cast(local_origin, local_translation, max_fraction)
                else {
                    return max_fraction;
                };
                let ray_hit = RayHit {
                    shape: shape_id,
                    body: shape.body,
                    point: body.pose.transform_point(hit.point),
                    normal: normalize(body.pose.rotate(hit.normal)),
                    fraction: hit.fraction,
                };
                callback(&ray_hit)
            });
    }

    /// The closest hit of a segment against the world.
    fn cast_segment_closest(
        &self,
        origin: Vec3,
        translation: Vec3,
        filter: &QueryFilter,
    ) -> Option<RayHit> {
        let mut closest = None;
        self.cast_segment(origin, translation, filter, |hit| {
            closest = Some(*hit);
            hit.fraction
        });
        closest
    }

    /// The shapes whose bounds overlap `aabb`, in handle order.
    pub fn overlap_aabb(&self, aabb: &Aabb, filter: &QueryFilter) -> Vec<ShapeId> {
        let mut found = Vec::new();
        self.broad_phase.query_all(aabb, |slot| {
            if let Some(id) = self.shapes.handle_at(slot as usize)
                && let Some(shape) = self.shapes.get(id)
            {
                let accepted = (shape.filter.category & filter.mask) != 0
                    && (shape.filter.mask & filter.category) != 0;
                if accepted && shape.aabb.overlaps(aabb) {
                    found.push(id);
                }
            }
            true
        });
        found.sort();
        found
    }
}
