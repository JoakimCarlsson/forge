//! The end of a step: moving bodies to their solved poses, measuring how still they are, sweeping
//! fast bodies, refreshing bounds and putting still islands to sleep.

use fr_core::Vec3;

use crate::body::{BodyId, BodyType};
use crate::constants::TIME_TO_SLEEP;
use crate::island::IslandId;
use crate::math::{abs, inv_rotate, modified_cross, mul_quat, normalize_quat, quat_vector};
use crate::solver::{BodyState, StepContext};
use crate::world::World;

impl World {
    /// Applies the sub-step displacements to the bodies, then sleeps still islands.
    pub(crate) fn finalize_bodies(
        &mut self,
        awake: &[BodyId],
        states: &[BodyState],
        ctx: &StepContext,
    ) {
        let time_step = ctx.dt;
        let inv_time_step = ctx.inv_dt;
        let enable_sleep = self.def.enable_sleep;
        let enable_continuous = self.def.enable_continuous;
        let mut awake_islands = vec![false; self.islands.capacity()];
        let mut split_candidate: Option<(IslandId, f32)> = None;
        let mut fast_bodies: Vec<BodyId> = Vec::new();
        for (&id, state) in awake.iter().zip(states) {
            let Some(body) = self.bodies.get_mut(id) else {
                continue;
            };
            let v = state.linear_velocity;
            let w = state.angular_velocity;
            let local_omega = inv_rotate(body.pose.rotation, w);
            let local_delta_rotation =
                inv_rotate(body.pose.rotation, quat_vector(state.delta_rotation));
            body.center += state.delta_position;
            body.pose.rotation = normalize_quat(mul_quat(state.delta_rotation, body.pose.rotation));
            let velocity_arc = modified_cross(abs(local_omega), body.max_extent);
            let max_velocity = v.length() + velocity_arc.length();
            let rotation_arc = modified_cross(abs(local_delta_rotation), body.max_extent);
            let max_delta_position = state.delta_position.length() + 2.0 * rotation_arc.length();
            let position_sleep_factor = 0.5;
            let sleep_velocity =
                max_velocity.max(position_sleep_factor * inv_time_step * max_delta_position);
            body.pose.position = body.center - body.pose.rotation * body.local_center;
            body.sleep_velocity = sleep_velocity;
            body.linear_velocity = v;
            body.angular_velocity = w;
            body.force = Vec3::ZERO;
            body.torque = Vec3::ZERO;
            body.is_fast = false;
            if !enable_sleep || !body.enable_sleep || sleep_velocity > body.sleep_threshold {
                body.sleep_time = 0.0;
                let max_motion = max_delta_position.max(max_velocity * time_step);
                if body.body_type == BodyType::Dynamic
                    && enable_continuous
                    && max_motion > body.safety_factor * body.min_extent
                {
                    body.is_fast = true;
                    fast_bodies.push(id);
                } else {
                    body.center0 = body.center;
                    body.rotation0 = body.pose.rotation;
                }
            } else {
                body.center0 = body.center;
                body.rotation0 = body.pose.rotation;
                body.sleep_time += time_step;
            }
            body.update_inverse_inertia_world();
            let island_id = body.island;
            let sleep_time = body.sleep_time;
            if let Some(island) = self.islands.get(island_id) {
                if sleep_time < TIME_TO_SLEEP {
                    awake_islands[island_id.index()] = true;
                } else if island.constraint_remove_count > 0 {
                    let better = match split_candidate {
                        None => true,
                        Some((best_id, best_time)) => {
                            sleep_time > best_time
                                || (sleep_time == best_time && island_id.index() > best_id.index())
                        }
                    };
                    if better {
                        split_candidate = Some((island_id, sleep_time));
                    }
                }
            }
        }
        for &id in &fast_bodies {
            self.solve_continuous(id, time_step);
        }
        for &id in awake {
            let is_fast = self.bodies.get(id).is_some_and(|b| b.is_fast);
            if is_fast {
                continue;
            }
            let shapes = self
                .bodies
                .get(id)
                .map(|b| b.shapes.clone())
                .unwrap_or_default();
            for shape in shapes {
                self.refresh_shape_bounds(shape);
            }
        }
        if enable_sleep {
            self.split_island = split_candidate.map(|(island, _)| island);
            let island_ids: Vec<IslandId> = self.islands.handles().collect();
            for island_id in island_ids {
                let island_awake = awake_islands
                    .get(island_id.index())
                    .copied()
                    .unwrap_or(false);
                let sleeping = self.islands.get(island_id).is_some_and(|i| i.asleep);
                if !island_awake && !sleeping {
                    self.try_sleep_island(island_id);
                }
            }
        }
    }
}
