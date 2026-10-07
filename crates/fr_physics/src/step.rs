//! Advancing the world by one step: finding new pairs, updating contacts, the soft step solver
//! with warm starting, relaxation and restitution, and finalizing bodies with sleeping and
//! continuous collision.

use fr_core::{Mat3, Vec3};

use crate::body::{Body, BodyId, BodyType};
use crate::constants::{MAX_RESTITUTION_ITERATIONS, MAX_ROTATION, SPECULATIVE_DISTANCE};
use crate::contact::{ContactChange, ContactId, ContactUpdateSettings};
use crate::contact_solver::{ConstraintBody, ContactConstraint};
use crate::joint::{Joint, JointId};
use crate::math::{integrate_rotation, inv_rotate, mul_add, mul_quat, solve3};
use crate::solver::{BodyState, StepContext};
use crate::world::{ContactEvent, World};

/// The solver arrays of one step.
struct Solve {
    /// The awake bodies in solver order.
    awake: Vec<BodyId>,
    /// The state of each awake body.
    states: Vec<BodyState>,
    /// The joints that take part in the step.
    joints: Vec<JointId>,
    /// The constraints of the touching contacts.
    constraints: Vec<ContactConstraint>,
    /// Whether any constraint has restitution.
    any_restitution: bool,
}

impl World {
    /// Advances the simulation by `dt` seconds using `sub_steps` sub-steps. A step of zero length
    /// only updates pairs and contacts.
    pub fn step(&mut self, dt: f32, sub_steps: usize) {
        self.contact_begin_events.clear();
        self.contact_end_events.clear();
        self.sensor_begin_events.clear();
        self.sensor_end_events.clear();
        self.update_pairs();
        let ctx = self.make_context(dt, sub_steps);
        self.last_context = Some(ctx);
        self.collide();
        if dt > 0.0 {
            self.step_count += 1;
            self.solve(&ctx, sub_steps.max(1));
        }
        self.update_sensors();
    }

    /// Creates contacts for the pairs of proxies whose bounds started to overlap.
    fn update_pairs(&mut self) {
        let moved = std::mem::take(&mut self.broad_phase.move_buffer);
        let mut candidates: Vec<(crate::shape::ShapeId, crate::shape::ShapeId)> = Vec::new();
        for &query_id in &moved {
            let Some(query) = self.shapes.get(query_id) else {
                continue;
            };
            let Some(body) = self.bodies.get(query.body) else {
                continue;
            };
            let query_type = body.body_type;
            let aabb = query.fat_aabb;
            let trees: &[BodyType] = if query_type == BodyType::Dynamic {
                &[BodyType::Dynamic, BodyType::Static, BodyType::Kinematic]
            } else {
                &[BodyType::Dynamic]
            };
            for &tree in trees {
                self.broad_phase.query_tree(tree, &aabb, |slot| {
                    let Some(found_id) = self.shapes.handle_at(slot as usize) else {
                        return true;
                    };
                    if found_id == query_id {
                        return true;
                    }
                    if let Some(found) = self.shapes.get(found_id)
                        && found.moved
                        && found_id.index() < query_id.index()
                    {
                        return true;
                    }
                    candidates.push((query_id, found_id));
                    true
                });
            }
        }
        for &query_id in &moved {
            if let Some(shape) = self.shapes.get_mut(query_id) {
                shape.moved = false;
            }
        }
        for (a, b) in candidates {
            let key = Self::pair_key(a, b);
            if self.pair_set.contains(&key) {
                continue;
            }
            if !self.should_create_pair(a, b) {
                continue;
            }
            self.pair_set.insert(key);
            self.create_contact(a, b);
        }
    }

    /// Whether two shapes with overlapping bounds should get a contact.
    fn should_create_pair(&self, a: crate::shape::ShapeId, b: crate::shape::ShapeId) -> bool {
        let (Some(shape_a), Some(shape_b)) = (self.shapes.get(a), self.shapes.get(b)) else {
            return false;
        };
        if shape_a.body == shape_b.body || shape_a.is_sensor || shape_b.is_sensor {
            return false;
        }
        if !shape_a.filter.should_collide(&shape_b.filter) {
            return false;
        }
        self.should_bodies_collide(shape_a.body, shape_b.body)
    }

    /// Updates every contact that has an awake body and handles the state changes.
    fn collide(&mut self) {
        let settings = ContactUpdateSettings {
            recycle_distance: self.def.contact_recycle_distance,
            recycle_distance_non_touching: self
                .def
                .contact_recycle_distance
                .min(SPECULATIVE_DISTANCE),
        };
        let ids: Vec<ContactId> = self.contacts.handles().collect();
        let mut changes: Vec<(ContactId, bool, ContactChange)> = Vec::with_capacity(ids.len());
        {
            let Self {
                contacts,
                shapes,
                bodies,
                ..
            } = self;
            for id in ids {
                let Some(contact) = contacts.get_mut(id) else {
                    continue;
                };
                let (Some(body_a), Some(body_b)) =
                    (bodies.get(contact.body_a), bodies.get(contact.body_b))
                else {
                    continue;
                };
                if !body_a.is_awake() && !body_b.is_awake() {
                    continue;
                }
                let (Some(shape_a), Some(shape_b)) =
                    (shapes.get(contact.shape_a), shapes.get(contact.shape_b))
                else {
                    continue;
                };
                let was_touching = contact.touching;
                let change = contact.collide(shape_a, shape_b, body_a, body_b, &settings);
                changes.push((id, was_touching, change));
            }
        }
        for (id, was_touching, change) in changes {
            match change {
                ContactChange::Disjoint => self.destroy_contact(id, false),
                ContactChange::Recycled => {}
                ContactChange::Updated { touching } => {
                    if touching && !was_touching {
                        self.begin_touch(id);
                    } else if !touching && was_touching {
                        self.end_touch(id);
                    }
                }
            }
        }
    }

    /// Links a contact that started touching and reports it.
    fn begin_touch(&mut self, id: ContactId) {
        let Some(contact) = self.contacts.get(id) else {
            return;
        };
        if contact.enable_events {
            self.contact_begin_events.push(ContactEvent {
                shape_a: contact.shape_a,
                shape_b: contact.shape_b,
            });
        }
        self.link_contact(id);
    }

    /// Unlinks a contact that stopped touching and reports it.
    fn end_touch(&mut self, id: ContactId) {
        let Some(contact) = self.contacts.get(id) else {
            return;
        };
        if contact.enable_events {
            self.contact_end_events.push(ContactEvent {
                shape_a: contact.shape_a,
                shape_b: contact.shape_b,
            });
        }
        self.unlink_contact(id);
    }

    /// Gathers the awake bodies, joints and touching contacts into solver arrays.
    fn gather(&mut self, ctx: &StepContext) -> Solve {
        let mut solve = Solve {
            awake: Vec::new(),
            states: Vec::new(),
            joints: Vec::new(),
            constraints: Vec::new(),
            any_restitution: false,
        };
        let ids: Vec<BodyId> = self.bodies.handles().collect();
        for id in ids {
            let Some(body) = self.bodies.get_mut(id) else {
                continue;
            };
            if body.is_awake() {
                body.solver_index = solve.awake.len();
                body.previous_pose = body.pose;
                solve.awake.push(id);
                solve.states.push(BodyState {
                    linear_velocity: body.linear_velocity,
                    angular_velocity: body.angular_velocity,
                    delta_position: Vec3::ZERO,
                    delta_rotation: fr_core::Quat::IDENTITY,
                    dynamic: body.is_dynamic(),
                });
            } else {
                body.solver_index = usize::MAX;
                body.previous_pose = body.pose;
            }
        }
        let joint_ids: Vec<JointId> = self.joints.handles().collect();
        for id in joint_ids {
            let Some(joint) = self.joints.get_mut(id) else {
                continue;
            };
            let (Some(body_a), Some(body_b)) =
                (self.bodies.get(joint.body_a), self.bodies.get(joint.body_b))
            else {
                continue;
            };
            if !(body_a.is_awake() || body_b.is_awake()) || !Joint::has_dynamic_body(body_a, body_b)
            {
                continue;
            }
            joint.prepare(body_a, body_b, ctx);
            solve.joints.push(id);
        }
        let contact_ids: Vec<ContactId> = self.contacts.handles().collect();
        for id in contact_ids {
            let Some(contact) = self.contacts.get(id) else {
                continue;
            };
            if !contact.touching {
                continue;
            }
            let (Some(body_a), Some(body_b)) = (
                self.bodies.get(contact.body_a),
                self.bodies.get(contact.body_b),
            ) else {
                continue;
            };
            if !(body_a.is_awake() || body_b.is_awake()) {
                continue;
            }
            let (constraint, has_restitution) = ContactConstraint::prepare(
                id,
                contact,
                &constraint_body(body_a),
                &constraint_body(body_b),
                &solve.states,
                ctx,
            );
            solve.any_restitution |= has_restitution;
            solve.constraints.push(constraint);
        }
        solve
    }

    /// Runs the solver for one step and finalizes the bodies.
    fn solve(&mut self, ctx: &StepContext, sub_steps: usize) {
        if let Some(island) = self.split_island.take() {
            self.split_island_by_id(island);
        }
        let mut solve = self.gather(ctx);
        if solve.awake.is_empty() {
            return;
        }
        for _ in 0..sub_steps {
            integrate_velocities(
                &self.bodies,
                &solve.awake,
                &mut solve.states,
                self.def.gravity,
                ctx.h,
            );
            for &id in &solve.joints {
                if let Some(joint) = self.joints.get_mut(id) {
                    joint.warm_start(&mut solve.states);
                }
            }
            for constraint in &solve.constraints {
                constraint.warm_start(&mut solve.states);
            }
            for &id in &solve.joints {
                if let Some(joint) = self.joints.get_mut(id) {
                    joint.solve(&mut solve.states, ctx, true);
                }
            }
            for constraint in &mut solve.constraints {
                constraint.push(&mut solve.states, ctx);
            }
            integrate_positions(&mut solve.states, ctx);
            for &id in &solve.joints {
                if let Some(joint) = self.joints.get_mut(id) {
                    joint.solve(&mut solve.states, ctx, false);
                }
            }
            for constraint in &mut solve.constraints {
                constraint.relax(&mut solve.states, ctx);
            }
        }
        let iterations = self
            .def
            .restitution_iterations
            .min(MAX_RESTITUTION_ITERATIONS);
        if solve.any_restitution {
            for _ in 0..iterations {
                for constraint in &mut solve.constraints {
                    constraint.apply_restitution(&mut solve.states, ctx);
                }
            }
        }
        for constraint in &solve.constraints {
            if let Some(contact) = self.contacts.get_mut(constraint.contact()) {
                constraint.store_impulses(contact);
            }
        }
        self.finalize_bodies(&solve.awake, &solve.states, ctx);
    }
}

/// The body data a contact constraint reads.
fn constraint_body(body: &Body) -> ConstraintBody {
    ConstraintBody {
        index: if body.is_awake() {
            body.solver_index
        } else {
            usize::MAX
        },
        inv_mass: body.inv_mass,
        inv_inertia: body.inv_inertia_world,
    }
}

/// Integrates the velocities into the sub-step displacement of every body.
fn integrate_positions(states: &mut [BodyState], ctx: &StepContext) {
    let h = ctx.h;
    let max_linear_speed = ctx.max_linear_speed;
    let max_angular_speed = MAX_ROTATION * ctx.inv_dt;
    let max_linear_squared = max_linear_speed * max_linear_speed;
    let max_angular_squared = max_angular_speed * max_angular_speed;
    for state in states.iter_mut() {
        let mut v = state.linear_velocity;
        let mut w = state.angular_velocity;
        if v.dot(v) > max_linear_squared {
            let ratio = max_linear_speed / v.length();
            v *= ratio;
        }
        if w.dot(w) > max_angular_squared {
            let ratio = max_angular_speed / w.length();
            w *= ratio;
        }
        state.linear_velocity = v;
        state.angular_velocity = w;
        state.delta_position = mul_add(state.delta_position, h, v);
        state.delta_rotation = integrate_rotation(state.delta_rotation, w * h);
    }
}

/// Integrates gravity, forces, damping and gyroscopic torque into the velocities of the awake
/// bodies.
fn integrate_velocities(
    bodies: &crate::slot::SlotMap<Body, crate::body::BodyTag>,
    awake: &[BodyId],
    states: &mut [BodyState],
    gravity: Vec3,
    h: f32,
) {
    for (&id, state) in awake.iter().zip(states.iter_mut()) {
        let Some(body) = bodies.get(id) else {
            continue;
        };
        let mut v = state.linear_velocity;
        let mut w = state.angular_velocity;
        let linear_damping = 1.0 / (1.0 + h * body.linear_damping);
        let angular_damping = 1.0 / (1.0 + h * body.angular_damping);
        let gravity_scale = if body.inv_mass > 0.0 {
            body.gravity_scale
        } else {
            0.0
        };
        let linear_velocity_delta =
            (body.force * (h * body.inv_mass)) + (gravity * (h * gravity_scale));
        v = mul_add(linear_velocity_delta, linear_damping, v);
        let angular_velocity_delta = (body.inv_inertia_world * body.torque) * h;
        w = mul_add(angular_velocity_delta, angular_damping, w);
        w = gyroscopic(body, state, w, h);
        state.linear_velocity = v;
        state.angular_velocity = w;
    }
}

/// Solves the gyroscopic torque equation `I (w2 - w1) + h cross(w2, I w2) = 0` by Newton
/// iterations in the local frame of the body and returns the new world angular velocity.
fn gyroscopic(body: &Body, state: &BodyState, w: Vec3, h: f32) -> Vec3 {
    let q0 = body.pose.rotation;
    let q = mul_quat(state.delta_rotation, q0);
    let inertia: Mat3 = body.inertia_local;
    let omega1 = inv_rotate(q, w);
    let mut omega2 = omega1;
    let i00 = inertia.x_axis.x;
    let i01 = inertia.y_axis.x;
    let i02 = inertia.z_axis.x;
    let i11 = inertia.y_axis.y;
    let i12 = inertia.z_axis.y;
    let i22 = inertia.z_axis.z;
    let w1 = omega2.x;
    let w2 = omega2.y;
    let w3 = omega2.z;
    let iw1 = i00 * w1 + i01 * w2 + i02 * w3;
    let iw2 = i01 * w1 + i11 * w2 + i12 * w3;
    let iw3 = i02 * w1 + i12 * w2 + i22 * w3;
    let dw = omega2 - omega1;
    let b = Vec3::new(
        i00 * dw.x + i01 * dw.y + i02 * dw.z + h * (w2 * iw3 - w3 * iw2),
        i01 * dw.x + i11 * dw.y + i12 * dw.z + h * (w3 * iw1 - w1 * iw3),
        i02 * dw.x + i12 * dw.y + i22 * dw.z + h * (w1 * iw2 - w2 * iw1),
    );
    let jacobian = Mat3::from_cols(
        Vec3::new(
            i00 + h * (w2 * i02 - w3 * i01),
            i01 + h * (w3 * i00 - w1 * i02 - iw3),
            i02 + h * (w1 * i01 - w2 * i00 + iw2),
        ),
        Vec3::new(
            i01 + h * (w2 * i12 - w3 * i11 + iw3),
            i11 + h * (w3 * i01 - w1 * i12),
            i12 + h * (w1 * i11 - w2 * i01 - iw1),
        ),
        Vec3::new(
            i02 + h * (w2 * i22 - w3 * i12 - iw2),
            i12 + h * (w3 * i02 - w1 * i22 + iw1),
            i22 + h * (w1 * i12 - w2 * i02),
        ),
    );
    omega2 -= solve3(jacobian, b);
    q * omega2
}
