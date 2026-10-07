//! Continuous collision of fast dynamic bodies against static shapes, by conservative
//! advancement along the motion of the step.
//!
//! This follows the structure of Box3D's `b3SolveContinuous` (find the earliest hit over the
//! shapes of the body, move the body back to it, give back the lost gravity), but replaces the
//! root finding time of impact with conservative advancement over GJK distances.

use fr_core::{Quat, Vec3};

use crate::body::{BodyId, BodyType};
use crate::constants::LINEAR_SLOP;
use crate::distance::{DistanceInput, SimplexCache, shape_distance};
use crate::geometry::Geometry;
use crate::math::{Pose, inv_mul_quat, normalize_quat, quat_vector};
use crate::world::World;

/// The motion of a body over a step: centre of mass and orientation at both ends.
#[derive(Clone, Copy, Debug)]
struct Sweep {
    /// The centre of mass at the start.
    c1: Vec3,
    /// The centre of mass at the end.
    c2: Vec3,
    /// The orientation at the start.
    q1: Quat,
    /// The orientation at the end.
    q2: Quat,
    /// The centre of mass in the frame of the body.
    local_center: Vec3,
}

impl Sweep {
    /// The orientation at `fraction`, interpolated along the shorter arc.
    fn rotation_at(&self, fraction: f32) -> Quat {
        let q1 = self.q1;
        let q2 = if q1.dot(self.q2) < 0.0 {
            -self.q2
        } else {
            self.q2
        };
        normalize_quat(Quat::from_xyzw(
            (1.0 - fraction) * q1.x + fraction * q2.x,
            (1.0 - fraction) * q1.y + fraction * q2.y,
            (1.0 - fraction) * q1.z + fraction * q2.z,
            (1.0 - fraction) * q1.w + fraction * q2.w,
        ))
    }

    /// The centre of mass at `fraction`.
    fn center_at(&self, fraction: f32) -> Vec3 {
        self.c1 + (self.c2 - self.c1) * fraction
    }

    /// The pose of the body origin at `fraction`.
    fn pose_at(&self, fraction: f32) -> Pose {
        let rotation = self.rotation_at(fraction);
        Pose::new(
            self.center_at(fraction) - rotation * self.local_center,
            rotation,
        )
    }
}

/// The first fraction of the sweep at which the moving shape comes within the slop of the
/// static shape, if it does so after the start and before `max_fraction`.
fn time_of_impact(
    static_geometry: &Geometry,
    static_pose: &Pose,
    moving_geometry: &Geometry,
    sweep: &Sweep,
    speed_bound: f32,
    max_fraction: f32,
) -> Option<f32> {
    if speed_bound < f32::EPSILON {
        return None;
    }
    let target = LINEAR_SLOP;
    let tolerance = 0.25 * LINEAR_SLOP;
    let mut cache = SimplexCache::default();
    let mut t = 0.0;
    for _ in 0..40 {
        let pose = sweep.pose_at(t);
        let input = DistanceInput {
            proxy_a: static_geometry.proxy(),
            proxy_b: moving_geometry.proxy(),
            transform: static_pose.inv_mul(&pose),
            use_radii: true,
        };
        let output = shape_distance(&input, &mut cache);
        if output.distance < target + tolerance {
            return if t > 0.0 { Some(t) } else { None };
        }
        t += (output.distance - target) / speed_bound;
        if t >= max_fraction {
            return None;
        }
    }
    Some(t)
}

impl World {
    /// Sweeps a fast dynamic body against the static shapes it passes and moves it back to the
    /// earliest hit.
    pub(crate) fn solve_continuous(&mut self, body_id: BodyId, dt: f32) {
        let Some(body) = self.bodies.get(body_id) else {
            return;
        };
        let sweep = Sweep {
            c1: body.center0,
            c2: body.center,
            q1: body.rotation0,
            q2: body.pose.rotation,
            local_center: body.local_center,
        };
        let safety_factor = body.safety_factor;
        let shapes = body.shapes.clone();
        let rotation_chord = 2.0 * quat_vector(inv_mul_quat(sweep.q1, sweep.q2)).length();
        let xf2 = body.pose;
        let mut fraction = 1.0_f32;
        for &shape_id in &shapes {
            let Some(shape) = self.shapes.get(shape_id) else {
                continue;
            };
            if shape.is_sensor {
                continue;
            }
            let geometry = shape.geometry.clone();
            let centroid = geometry.local_centroid();
            let xf1 = sweep.pose_at(0.0);
            let centroid1 = xf1.transform_point(centroid);
            let centroid2 = xf2.transform_point(centroid);
            let radius = geometry.margin_radius();
            let min_extent = geometry.extent(centroid).min;
            let max_motion = (centroid1 - centroid2).length() + rotation_chord * radius;
            if max_motion <= safety_factor * min_extent {
                continue;
            }
            let box1 = shape.aabb;
            let box2 = geometry
                .compute_aabb(&xf2)
                .inflate(crate::constants::SPECULATIVE_DISTANCE);
            let swept = box1.union(&box2);
            let filter = shape.filter;
            let reach = geometry.extent(sweep.local_center).max.length();
            let speed_bound = (sweep.c2 - sweep.c1).length() + rotation_chord * reach;
            let mut candidates: Vec<u32> = Vec::new();
            self.broad_phase
                .query_tree(BodyType::Static, &swept, |slot| {
                    candidates.push(slot);
                    true
                });
            for slot in candidates {
                let Some(other_id) = self.shapes.handle_at(slot as usize) else {
                    continue;
                };
                let Some(other) = self.shapes.get(other_id) else {
                    continue;
                };
                if other.is_sensor || other.body == body_id || !filter.should_collide(&other.filter)
                {
                    continue;
                }
                if !self.should_bodies_collide(body_id, other.body) {
                    continue;
                }
                let Some(other_body) = self.bodies.get(other.body) else {
                    continue;
                };
                if let Some(hit) = time_of_impact(
                    &other.geometry,
                    &other_body.pose,
                    &geometry,
                    &sweep,
                    speed_bound,
                    fraction,
                ) && 0.0 < hit
                    && hit < fraction
                {
                    fraction = hit;
                }
            }
        }
        if fraction < 1.0 {
            let rotation = sweep.rotation_at(fraction);
            let center = sweep.center_at(fraction);
            let gravity = self.def.gravity;
            if let Some(body) = self.bodies.get_mut(body_id) {
                body.pose = Pose::new(center - rotation * body.local_center, rotation);
                body.center = center;
                body.rotation0 = rotation;
                body.center0 = center;
                body.update_inverse_inertia_world();
                let time_loss = (1.0 - fraction) * dt;
                body.linear_velocity += gravity * (-time_loss * body.gravity_scale);
            }
        } else if let Some(body) = self.bodies.get_mut(body_id) {
            body.rotation0 = body.pose.rotation;
            body.center0 = body.center;
        }
        for shape_id in shapes {
            self.refresh_shape_bounds(shape_id);
        }
    }
}
