//! The weld joint: locks the relative position and rotation of two frames, optionally as a
//! spring.

use fr_core::{Mat3, Quat, Vec3};

use crate::joint::JointSim;
use crate::joint::spherical::point_constraint_matrix;
use crate::math::{
    Softness, delta_quat_to_rotation, dot_quat, inv_mul_quat, invert_matrix, mul_add, mul_quat,
    mul_sub, negate_quat, solve3,
};
use crate::solver::{BodyState, StepContext, load_state, store_state};

/// A weld. Both stiffnesses are rigid at zero hertz.
#[derive(Clone, Debug)]
pub struct WeldJoint {
    /// The stiffness of the linear constraint in hertz; zero is rigid.
    pub linear_hertz: f32,
    /// The damping ratio of the linear spring.
    pub linear_damping_ratio: f32,
    /// The stiffness of the angular constraint in hertz; zero is rigid.
    pub angular_hertz: f32,
    /// The damping ratio of the angular spring.
    pub angular_damping_ratio: f32,
    /// The impulse of the linear constraint.
    pub(crate) linear_impulse: Vec3,
    /// The impulse of the angular constraint.
    pub(crate) angular_impulse: Vec3,
    /// The softness of the linear constraint for this step.
    pub(crate) linear_spring: Softness,
    /// The softness of the angular constraint for this step.
    pub(crate) angular_spring: Softness,
    /// The effective mass of the angular constraint.
    pub(crate) angular_mass: Mat3,
}

impl Default for WeldJoint {
    /// A rigid weld.
    fn default() -> Self {
        Self {
            linear_hertz: 0.0,
            linear_damping_ratio: 0.0,
            angular_hertz: 0.0,
            angular_damping_ratio: 0.0,
            linear_impulse: Vec3::ZERO,
            angular_impulse: Vec3::ZERO,
            linear_spring: Softness::RIGID,
            angular_spring: Softness::RIGID,
            angular_mass: Mat3::ZERO,
        }
    }
}

impl WeldJoint {
    /// Computes the effective masses and softness for the step.
    pub(crate) fn prepare(&mut self, sim: &JointSim, ctx: &StepContext) {
        self.angular_mass = invert_matrix(sim.inv_ia + sim.inv_ib);
        self.linear_spring = if self.linear_hertz == 0.0 {
            sim.constraint_softness
        } else {
            Softness::new(self.linear_hertz, self.linear_damping_ratio, ctx.h)
        };
        self.angular_spring = if self.angular_hertz == 0.0 {
            sim.constraint_softness
        } else {
            Softness::new(self.angular_hertz, self.angular_damping_ratio, ctx.h)
        };
        if !ctx.enable_warm_starting {
            self.linear_impulse = Vec3::ZERO;
            self.angular_impulse = Vec3::ZERO;
        }
    }

    /// Applies the accumulated impulses to the bodies.
    pub(crate) fn warm_start(&mut self, sim: &JointSim, states: &mut [BodyState]) {
        let mut state_a = load_state(states, sim.index_a);
        let mut state_b = load_state(states, sim.index_b);
        let r_a = state_a.delta_rotation * sim.frame_a.position;
        let r_b = state_b.delta_rotation * sim.frame_b.position;
        state_a.linear_velocity =
            mul_sub(state_a.linear_velocity, sim.inv_mass_a, self.linear_impulse);
        state_a.angular_velocity -=
            sim.inv_ia * (r_a.cross(self.linear_impulse) + self.angular_impulse);
        state_b.linear_velocity =
            mul_add(state_b.linear_velocity, sim.inv_mass_b, self.linear_impulse);
        state_b.angular_velocity +=
            sim.inv_ib * (r_b.cross(self.linear_impulse) + self.angular_impulse);
        store_state(states, sim.index_a, &state_a);
        store_state(states, sim.index_b, &state_b);
    }

    /// Solves the angular and linear constraints for one iteration.
    pub(crate) fn solve(
        &mut self,
        sim: &JointSim,
        states: &mut [BodyState],
        _ctx: &StepContext,
        use_bias: bool,
    ) {
        let mut state_a = load_state(states, sim.index_a);
        let mut state_b = load_state(states, sim.index_b);
        let mut v_a = state_a.linear_velocity;
        let mut w_a = state_a.angular_velocity;
        let mut v_b = state_b.linear_velocity;
        let mut w_b = state_b.angular_velocity;
        let (i_a, i_b) = (sim.inv_ia, sim.inv_ib);
        let quat_a = mul_quat(state_a.delta_rotation, sim.frame_a.rotation);
        let mut quat_b = mul_quat(state_b.delta_rotation, sim.frame_b.rotation);
        if dot_quat(quat_a, quat_b) < 0.0 {
            quat_b = negate_quat(quat_b);
        }
        let rel_q = inv_mul_quat(quat_a, quat_b);

        if !sim.fixed_rotation {
            let mut bias = Vec3::ZERO;
            let mut mass_scale = 1.0;
            let mut impulse_scale = 0.0;
            if use_bias || self.angular_hertz > 0.0 {
                let delta_rotation = delta_quat_to_rotation(rel_q, Quat::IDENTITY);
                let c = -(quat_a * delta_rotation);
                bias = c * self.angular_spring.bias_rate;
                mass_scale = self.angular_spring.mass_scale;
                impulse_scale = self.angular_spring.impulse_scale;
            }
            let cdot = w_b - w_a;
            let impulse = mul_sub(
                (self.angular_mass * (cdot + bias)) * -mass_scale,
                impulse_scale,
                self.angular_impulse,
            );
            self.angular_impulse += impulse;
            w_a -= i_a * impulse;
            w_b += i_b * impulse;
        }

        {
            let r_a = state_a.delta_rotation * sim.frame_a.position;
            let r_b = state_b.delta_rotation * sim.frame_b.position;
            let cdot = (v_b + w_b.cross(r_b)) - (v_a + w_a.cross(r_a));
            let mut bias = Vec3::ZERO;
            let mut mass_scale = 1.0;
            let mut impulse_scale = 0.0;
            if use_bias || self.linear_hertz > 0.0 {
                let dc_a = state_a.delta_position;
                let dc_b = state_b.delta_position;
                let separation = ((dc_b - dc_a) + (r_b - r_a)) + sim.delta_center;
                bias = separation * self.linear_spring.bias_rate;
                mass_scale = self.linear_spring.mass_scale;
                impulse_scale = self.linear_spring.impulse_scale;
            }
            let k = point_constraint_matrix(r_a, r_b, sim);
            let b = solve3(k, cdot + bias);
            let impulse = mul_sub(b * -mass_scale, impulse_scale, self.linear_impulse);
            self.linear_impulse += impulse;
            v_a = mul_sub(v_a, sim.inv_mass_a, impulse);
            w_a -= i_a * r_a.cross(impulse);
            v_b = mul_add(v_b, sim.inv_mass_b, impulse);
            w_b += i_b * r_b.cross(impulse);
        }

        state_a.linear_velocity = v_a;
        state_a.angular_velocity = w_a;
        state_b.linear_velocity = v_b;
        state_b.angular_velocity = w_b;
        store_state(states, sim.index_a, &state_a);
        store_state(states, sim.index_b, &state_b);
    }
}
