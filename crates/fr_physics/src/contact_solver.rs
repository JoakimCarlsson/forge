//! The soft step contact solver: one constraint per touching contact with up to four normal
//! points, a central friction constraint, twist friction and rolling resistance.
//!
//! Within a sub-step the solve stage pushes bodies apart with the soft contact bias, the relax
//! stage solves again without bias and adds friction, and restitution runs once at the end.

use fr_core::{Mat3, Vec3};

use crate::constants::{MAX_MANIFOLD_POINTS, MIN_FRICTION_WEIGHT, SPECULATIVE_DISTANCE};
use crate::contact::{Contact, ContactId};
use crate::math::{Softness, invert_matrix, invert2, mul_add, mul_sub, perp};
use crate::solver::{BodyState, StepContext, load_state, store_state};

/// A normal point of a contact constraint.
#[derive(Clone, Copy, Debug, Default)]
struct ConstraintPoint {
    /// The anchor on A relative to its centre of mass.
    r_a: Vec3,
    /// The anchor on B relative to its centre of mass.
    r_b: Vec3,
    /// The separation with the movement of the anchors subtracted.
    base_separation: f32,
    /// The approach velocity before the solve.
    relative_velocity: f32,
    /// The accumulated normal impulse.
    normal_impulse: f32,
    /// The sum of the normal impulses of every iteration.
    total_normal_impulse: f32,
    /// The impulse restitution has added.
    restitution_impulse: f32,
    /// The effective mass along the normal.
    normal_mass: f32,
    /// The distance of the anchor from the friction centre.
    lever_arm: f32,
}

/// The solver constraint of a touching contact.
#[derive(Clone, Debug)]
pub(crate) struct ContactConstraint {
    /// The contact this constraint was made from.
    contact: ContactId,
    /// The solver index of body A, or the null index.
    index_a: usize,
    /// The solver index of body B, or the null index.
    index_b: usize,
    /// The inverse mass of A.
    inv_mass_a: f32,
    /// The inverse mass of B.
    inv_mass_b: f32,
    /// The world inverse inertia of A.
    inv_ia: Mat3,
    /// The world inverse inertia of B.
    inv_ib: Mat3,
    /// The effective mass of rolling resistance.
    rolling_mass: Mat3,
    /// The softness of the contact.
    softness: Softness,
    /// The friction coefficient.
    friction: f32,
    /// The restitution.
    restitution: f32,
    /// The rolling resistance coefficient.
    rolling_resistance: f32,
    /// The unit normal from A to B.
    normal: Vec3,
    /// The first friction direction.
    tangent1: Vec3,
    /// The second friction direction.
    tangent2: Vec3,
    /// The surface velocity along the first tangent.
    tangent_velocity1: f32,
    /// The surface velocity along the second tangent.
    tangent_velocity2: f32,
    /// The friction centre relative to A.
    center_a: Vec3,
    /// The friction centre relative to B.
    center_b: Vec3,
    /// The accumulated friction impulse along the two tangents.
    friction_impulse: (f32, f32),
    /// The effective mass of friction as the symmetric matrix `(xx, xy, yy)`.
    tangent_mass: (f32, f32, f32),
    /// The effective mass of twist friction.
    twist_mass: f32,
    /// The accumulated twist friction impulse.
    twist_impulse: f32,
    /// The accumulated rolling resistance impulse.
    rolling_impulse: Vec3,
    /// The normal points.
    points: [ConstraintPoint; MAX_MANIFOLD_POINTS],
    /// The number of points in use.
    count: usize,
}

/// The body data a constraint needs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ConstraintBody {
    /// The solver index of the body, or the null index when it does not move.
    pub(crate) index: usize,
    /// The inverse mass.
    pub(crate) inv_mass: f32,
    /// The world inverse inertia.
    pub(crate) inv_inertia: Mat3,
}

impl ContactConstraint {
    /// Builds the constraint of a touching contact and reports whether it has restitution.
    pub(crate) fn prepare(
        id: ContactId,
        contact: &Contact,
        body_a: &ConstraintBody,
        body_b: &ConstraintBody,
        states: &[BodyState],
        ctx: &StepContext,
    ) -> (Self, bool) {
        let warm_start_scale = if ctx.enable_warm_starting { 1.0 } else { 0.0 };
        let inv_tau = 1.0 / SPECULATIVE_DISTANCE;
        let manifold = &contact.manifold;
        let (m_a, i_a) = (body_a.inv_mass, body_a.inv_inertia);
        let (m_b, i_b) = (body_b.inv_mass, body_b.inv_inertia);
        let have_restitution = contact.restitution > 0.0;
        let normal = manifold.normal;
        let tangent1 = perp(normal);
        let tangent2 = tangent1.cross(normal);
        let mut constraint = Self {
            contact: id,
            index_a: body_a.index,
            index_b: body_b.index,
            inv_mass_a: m_a,
            inv_mass_b: m_b,
            inv_ia: i_a,
            inv_ib: i_b,
            rolling_mass: invert_matrix(i_a + i_b),
            softness: if contact.is_static {
                ctx.static_softness
            } else {
                ctx.contact_softness
            },
            friction: contact.friction,
            restitution: contact.restitution,
            rolling_resistance: contact.rolling_resistance,
            normal,
            tangent1,
            tangent2,
            tangent_velocity1: contact.tangent_velocity.dot(tangent1),
            tangent_velocity2: contact.tangent_velocity.dot(tangent2),
            center_a: Vec3::ZERO,
            center_b: Vec3::ZERO,
            friction_impulse: (
                warm_start_scale * manifold.friction_impulse.dot(tangent1),
                warm_start_scale * manifold.friction_impulse.dot(tangent2),
            ),
            tangent_mass: (0.0, 0.0, 0.0),
            twist_mass: 0.0,
            twist_impulse: warm_start_scale * manifold.twist_impulse,
            rolling_impulse: manifold.rolling_impulse * warm_start_scale,
            points: [ConstraintPoint::default(); MAX_MANIFOLD_POINTS],
            count: manifold.count,
        };
        let (mut v_a, mut w_a, mut v_b, mut w_b) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
        if have_restitution {
            let state_a = load_state(states, body_a.index);
            let state_b = load_state(states, body_b.index);
            v_a = state_a.linear_velocity;
            w_a = state_a.angular_velocity;
            v_b = state_b.linear_velocity;
            w_b = state_b.angular_velocity;
        }
        let mut center_a = Vec3::ZERO;
        let mut center_b = Vec3::ZERO;
        let mut total_weight = 0.0;
        for index in 0..manifold.count {
            let mp = &manifold.points[index];
            let cp = &mut constraint.points[index];
            cp.r_a = mp.anchor_a;
            cp.r_b = mp.anchor_b;
            let s = mp.separation;
            cp.base_separation = s - (cp.r_b - cp.r_a).dot(normal);
            cp.normal_impulse = warm_start_scale * mp.normal_impulse;
            let rn_a = cp.r_a.cross(normal);
            let rn_b = cp.r_b.cross(normal);
            let k_normal = m_a + m_b + rn_a.dot(i_a * rn_a) + rn_b.dot(i_b * rn_b);
            cp.normal_mass = if k_normal > 0.0 { 1.0 / k_normal } else { 0.0 };
            if have_restitution {
                let vr_a = v_a + w_a.cross(cp.r_a);
                let vr_b = v_b + w_b.cross(cp.r_b);
                cp.relative_velocity = normal.dot(vr_b - vr_a);
            }
            let weight = (2.0 - s * inv_tau).clamp(MIN_FRICTION_WEIGHT, 1.0);
            center_a = mul_add(center_a, weight, cp.r_a);
            center_b = mul_add(center_b, weight, cp.r_b);
            total_weight += weight;
        }
        let inv_weight = 1.0 / total_weight;
        center_a *= inv_weight;
        center_b *= inv_weight;
        constraint.center_a = center_a;
        constraint.center_b = center_b;
        for cp in constraint.points.iter_mut().take(manifold.count) {
            cp.lever_arm = (cp.r_a - center_a).length();
        }
        let rt_a1 = center_a.cross(tangent1);
        let rt_a2 = center_a.cross(tangent2);
        let rt_b1 = center_b.cross(tangent1);
        let rt_b2 = center_b.cross(tangent2);
        let kxx = m_a + m_b + rt_a1.dot(i_a * rt_a1) + rt_b1.dot(i_b * rt_b1);
        let kyy = m_a + m_b + rt_a2.dot(i_a * rt_a2) + rt_b2.dot(i_b * rt_b2);
        let kxy = rt_a1.dot(i_a * rt_a2) + rt_b1.dot(i_b * rt_b2);
        constraint.tangent_mass = invert2(kxx, kxy, kyy);
        let k = normal.dot((i_a + i_b) * normal);
        constraint.twist_mass = if k > 0.0 { 1.0 / k } else { 0.0 };
        (constraint, have_restitution)
    }

    /// Applies the impulses of the previous step.
    pub(crate) fn warm_start(&self, states: &mut [BodyState]) {
        let mut state_a = load_state(states, self.index_a);
        let mut state_b = load_state(states, self.index_b);
        let normal = self.normal;
        let mut total_normal_impulse = 0.0;
        let mut moment_a = Vec3::ZERO;
        let mut moment_b = Vec3::ZERO;
        for cp in self.points.iter().take(self.count) {
            total_normal_impulse += cp.normal_impulse;
            moment_a = mul_add(moment_a, cp.normal_impulse, cp.r_a);
            moment_b = mul_add(moment_b, cp.normal_impulse, cp.r_b);
        }
        let friction_impulse = mul_add(
            self.tangent1 * self.friction_impulse.0,
            self.friction_impulse.1,
            self.tangent2,
        );
        let linear_impulse = mul_add(friction_impulse, total_normal_impulse, normal);
        let twist_impulse = normal * self.twist_impulse;
        let angular_a =
            (moment_a.cross(normal) + self.center_a.cross(friction_impulse)) + twist_impulse;
        let angular_b =
            (moment_b.cross(normal) + self.center_b.cross(friction_impulse)) + twist_impulse;
        let angular_impulse_a = angular_a + self.rolling_impulse;
        let angular_impulse_b = angular_b + self.rolling_impulse;
        if state_a.dynamic {
            state_a.linear_velocity =
                mul_sub(state_a.linear_velocity, self.inv_mass_a, linear_impulse);
            state_a.angular_velocity -= self.inv_ia * angular_impulse_a;
        }
        if state_b.dynamic {
            state_b.linear_velocity =
                mul_add(state_b.linear_velocity, self.inv_mass_b, linear_impulse);
            state_b.angular_velocity += self.inv_ib * angular_impulse_b;
        }
        store_state(states, self.index_a, &state_a);
        store_state(states, self.index_b, &state_b);
    }

    /// Solves the non penetration constraints with the soft bias; no friction, no restitution.
    pub(crate) fn push(&mut self, states: &mut [BodyState], ctx: &StepContext) {
        let mut state_a = load_state(states, self.index_a);
        let mut state_b = load_state(states, self.index_b);
        let (mut v_a, mut w_a) = (state_a.linear_velocity, state_a.angular_velocity);
        let (mut v_b, mut w_b) = (state_b.linear_velocity, state_b.angular_velocity);
        let (dq_a, dq_b) = (state_a.delta_rotation, state_b.delta_rotation);
        let dp = state_b.delta_position - state_a.delta_position;
        let (m_a, i_a, m_b, i_b) = (self.inv_mass_a, self.inv_ia, self.inv_mass_b, self.inv_ib);
        let softness = self.softness;
        let normal = self.normal;
        for cp in self.points.iter_mut().take(self.count) {
            let r_a = cp.r_a;
            let r_b = cp.r_b;
            let ds = dp + ((dq_b * r_b) - (dq_a * r_a));
            let s = ds.dot(normal) + cp.base_separation;
            let (velocity_bias, mass_scale, impulse_scale) = if s > 0.0 {
                (s * ctx.inv_h, 1.0, 0.0)
            } else {
                (
                    (softness.mass_scale * softness.bias_rate * s).max(-ctx.contact_speed),
                    softness.mass_scale,
                    softness.impulse_scale,
                )
            };
            let vr_a = v_a + w_a.cross(r_a);
            let vr_b = v_b + w_b.cross(r_b);
            let vn = (vr_b - vr_a).dot(normal);
            let mut delta_impulse = -cp.normal_mass * (mass_scale * vn + velocity_bias)
                - impulse_scale * cp.normal_impulse;
            let new_impulse = (cp.normal_impulse + delta_impulse).max(0.0);
            delta_impulse = new_impulse - cp.normal_impulse;
            cp.normal_impulse = new_impulse;
            let p = normal * delta_impulse;
            v_a = mul_sub(v_a, m_a, p);
            w_a -= i_a * r_a.cross(p);
            v_b = mul_add(v_b, m_b, p);
            w_b += i_b * r_b.cross(p);
        }
        state_a.linear_velocity = v_a;
        state_a.angular_velocity = w_a;
        state_b.linear_velocity = v_b;
        state_b.angular_velocity = w_b;
        store_state(states, self.index_a, &state_a);
        store_state(states, self.index_b, &state_b);
    }

    /// Solves normal, twist, rolling and friction constraints without bias.
    pub(crate) fn relax(&mut self, states: &mut [BodyState], ctx: &StepContext) {
        let mut state_a = load_state(states, self.index_a);
        let mut state_b = load_state(states, self.index_b);
        let (mut v_a, mut w_a) = (state_a.linear_velocity, state_a.angular_velocity);
        let (mut v_b, mut w_b) = (state_b.linear_velocity, state_b.angular_velocity);
        let (dq_a, dq_b) = (state_a.delta_rotation, state_b.delta_rotation);
        let dp = state_b.delta_position - state_a.delta_position;
        let (m_a, i_a, m_b, i_b) = (self.inv_mass_a, self.inv_ia, self.inv_mass_b, self.inv_ib);
        let friction = self.friction;
        let normal = self.normal;
        let mut total_normal_impulse = 0.0;
        let mut total_twist_limit = 0.0;
        for cp in self.points.iter_mut().take(self.count) {
            let r_a = cp.r_a;
            let r_b = cp.r_b;
            let ds = dp + ((dq_b * r_b) - (dq_a * r_a));
            let s = ds.dot(normal) + cp.base_separation;
            let velocity_bias = if s > 0.0 { s * ctx.inv_h } else { 0.0 };
            let vr_a = v_a + w_a.cross(r_a);
            let vr_b = v_b + w_b.cross(r_b);
            let vn = (vr_b - vr_a).dot(normal);
            let mut delta_impulse = -cp.normal_mass * (vn + velocity_bias);
            let new_impulse = (cp.normal_impulse + delta_impulse).max(0.0);
            delta_impulse = new_impulse - cp.normal_impulse;
            cp.normal_impulse = new_impulse;
            cp.total_normal_impulse += new_impulse;
            total_normal_impulse += new_impulse;
            total_twist_limit += cp.lever_arm * new_impulse;
            let p = normal * delta_impulse;
            v_a = mul_sub(v_a, m_a, p);
            w_a -= i_a * r_a.cross(p);
            v_b = mul_add(v_b, m_b, p);
            w_b += i_b * r_b.cross(p);
        }
        {
            let twist_speed = normal.dot(w_b - w_a);
            let max_impulse = friction * total_twist_limit;
            let delta_impulse = -self.twist_mass * twist_speed;
            let old_impulse = self.twist_impulse;
            self.twist_impulse = (old_impulse + delta_impulse).clamp(-max_impulse, max_impulse);
            let delta_impulse = self.twist_impulse - old_impulse;
            w_a -= i_a * (normal * delta_impulse);
            w_b += i_b * (normal * delta_impulse);
        }
        if self.rolling_resistance > 0.0 {
            let mut delta_impulse = -(self.rolling_mass * (w_b - w_a));
            let old_impulse = self.rolling_impulse;
            self.rolling_impulse = old_impulse + delta_impulse;
            let max_impulse = self.rolling_resistance * total_normal_impulse;
            let mag_squared = self.rolling_impulse.dot(self.rolling_impulse);
            if mag_squared > max_impulse * max_impulse + f32::EPSILON {
                self.rolling_impulse *= max_impulse / mag_squared.sqrt();
            }
            delta_impulse = self.rolling_impulse - old_impulse;
            w_a -= i_a * delta_impulse;
            w_b += i_b * delta_impulse;
        }
        {
            let tangent1 = self.tangent1;
            let tangent2 = self.tangent2;
            let r_a = self.center_a;
            let r_b = self.center_b;
            let vr_a = v_a + w_a.cross(r_a);
            let vr_b = v_b + w_b.cross(r_b);
            let vr = vr_b - vr_a;
            let vt = (
                vr.dot(tangent1) - self.tangent_velocity1,
                vr.dot(tangent2) - self.tangent_velocity2,
            );
            let (txx, txy, tyy) = self.tangent_mass;
            let tm = (txx * vt.0 + txy * vt.1, txy * vt.0 + tyy * vt.1);
            let delta = (-tm.0, -tm.1);
            let mut new_impulse = (
                self.friction_impulse.0 + delta.0,
                self.friction_impulse.1 + delta.1,
            );
            let max_impulse = friction * total_normal_impulse;
            let length_squared = new_impulse.0 * new_impulse.0 + new_impulse.1 * new_impulse.1;
            if length_squared > max_impulse * max_impulse {
                let scale = max_impulse / length_squared.sqrt();
                new_impulse.0 *= scale;
                new_impulse.1 *= scale;
            }
            let delta = (
                new_impulse.0 - self.friction_impulse.0,
                new_impulse.1 - self.friction_impulse.1,
            );
            self.friction_impulse = new_impulse;
            let p = (tangent1 * delta.0) + (tangent2 * delta.1);
            v_a = mul_sub(v_a, m_a, p);
            w_a -= i_a * r_a.cross(p);
            v_b = mul_add(v_b, m_b, p);
            w_b += i_b * r_b.cross(p);
        }
        state_a.linear_velocity = v_a;
        state_a.angular_velocity = w_a;
        state_b.linear_velocity = v_b;
        state_b.angular_velocity = w_b;
        store_state(states, self.index_a, &state_a);
        store_state(states, self.index_b, &state_b);
    }

    /// Applies Poisson restitution to the points that were approaching fast.
    pub(crate) fn apply_restitution(&mut self, states: &mut [BodyState], ctx: &StepContext) {
        let restitution = self.restitution;
        if restitution == 0.0 {
            return;
        }
        let threshold = ctx.restitution_threshold;
        let mut state_a = load_state(states, self.index_a);
        let mut state_b = load_state(states, self.index_b);
        let (mut v_a, mut w_a) = (state_a.linear_velocity, state_a.angular_velocity);
        let (mut v_b, mut w_b) = (state_b.linear_velocity, state_b.angular_velocity);
        let (dq_a, dq_b) = (state_a.delta_rotation, state_b.delta_rotation);
        let dp = state_b.delta_position - state_a.delta_position;
        let (m_a, i_a, m_b, i_b) = (self.inv_mass_a, self.inv_ia, self.inv_mass_b, self.inv_ib);
        let normal = self.normal;
        for cp in self.points.iter_mut().take(self.count) {
            let r_a = cp.r_a;
            let r_b = cp.r_b;
            let compression_impulse = cp.total_normal_impulse - cp.restitution_impulse;
            let armed = cp.relative_velocity < -threshold && compression_impulse > 0.0;
            let velocity_bias = if armed {
                restitution * cp.relative_velocity
            } else {
                let ds = dp + ((dq_b * r_b) - (dq_a * r_a));
                let s = ds.dot(normal) + cp.base_separation;
                if s > 0.0 { s * ctx.inv_h } else { 0.0 }
            };
            let vr_a = v_a + w_a.cross(r_a);
            let vr_b = v_b + w_b.cross(r_b);
            let vn = (vr_b - vr_a).dot(normal);
            let mut impulse = -cp.normal_mass * (vn + velocity_bias);
            let new_impulse = (cp.normal_impulse + impulse).max(0.0);
            impulse = new_impulse - cp.normal_impulse;
            let approach_impulse = (-cp.normal_mass * vn).max(0.0).min(impulse.max(0.0));
            if armed {
                let allowance =
                    restitution * (compression_impulse + approach_impulse) - cp.restitution_impulse;
                impulse = impulse.min(approach_impulse + allowance.max(0.0));
            }
            cp.normal_impulse += impulse;
            cp.restitution_impulse += impulse - approach_impulse;
            cp.total_normal_impulse += impulse;
            let p = normal * impulse;
            v_a = mul_sub(v_a, m_a, p);
            w_a -= i_a * r_a.cross(p);
            v_b = mul_add(v_b, m_b, p);
            w_b += i_b * r_b.cross(p);
        }
        state_a.linear_velocity = v_a;
        state_a.angular_velocity = w_a;
        state_b.linear_velocity = v_b;
        state_b.angular_velocity = w_b;
        store_state(states, self.index_a, &state_a);
        store_state(states, self.index_b, &state_b);
    }

    /// Writes the impulses back to the contact for the next step's warm start.
    pub(crate) fn store_impulses(&self, contact: &mut Contact) {
        let manifold = &mut contact.manifold;
        manifold.twist_impulse = self.twist_impulse;
        manifold.friction_impulse =
            (self.tangent1 * self.friction_impulse.0) + (self.tangent2 * self.friction_impulse.1);
        manifold.rolling_impulse = self.rolling_impulse;
        for (cp, mp) in self
            .points
            .iter()
            .zip(manifold.points.iter_mut())
            .take(self.count)
        {
            mp.normal_impulse = cp.normal_impulse;
            mp.total_normal_impulse = cp.total_normal_impulse;
        }
    }

    /// The contact the constraint belongs to.
    pub(crate) fn contact(&self) -> ContactId {
        self.contact
    }
}
