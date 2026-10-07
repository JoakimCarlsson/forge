//! The distance joint: keeps two anchors at a length, as a rigid rod or a spring, with optional
//! limits and a motor.

use fr_core::Vec3;

use crate::joint::JointSim;
use crate::math::{Softness, mul_add, mul_sub, normalize};
use crate::solver::{BodyState, StepContext, load_state, store_state};

/// A distance constraint between the anchors of the two joint frames.
#[derive(Clone, Debug)]
pub struct DistanceJoint {
    /// The rest length.
    pub length: f32,
    /// Whether the length is a spring rather than rigid.
    pub enable_spring: bool,
    /// The stiffness of the spring in hertz.
    pub hertz: f32,
    /// The damping ratio of the spring.
    pub damping_ratio: f32,
    /// The lowest spring force, negative to resist compression.
    pub lower_spring_force: f32,
    /// The highest spring force.
    pub upper_spring_force: f32,
    /// Whether the length is limited to `min_length` and `max_length` when springy.
    pub enable_limit: bool,
    /// The shortest length when limited.
    pub min_length: f32,
    /// The longest length when limited.
    pub max_length: f32,
    /// Whether the motor drives the length rate.
    pub enable_motor: bool,
    /// The largest force of the motor.
    pub max_motor_force: f32,
    /// The target rate of length change of the motor.
    pub motor_speed: f32,
    /// The impulse of the rigid constraint or the spring.
    pub(crate) impulse: f32,
    /// The impulse of the lower limit.
    pub(crate) lower_impulse: f32,
    /// The impulse of the upper limit.
    pub(crate) upper_impulse: f32,
    /// The impulse of the motor.
    pub(crate) motor_impulse: f32,
    /// The effective mass along the axis.
    pub(crate) axial_mass: f32,
    /// The softness of the spring for this step.
    pub(crate) distance_softness: Softness,
}

impl Default for DistanceJoint {
    /// A rigid rod of one metre.
    fn default() -> Self {
        Self {
            length: 1.0,
            enable_spring: false,
            hertz: 0.0,
            damping_ratio: 0.0,
            lower_spring_force: -f32::MAX,
            upper_spring_force: f32::MAX,
            enable_limit: false,
            min_length: crate::constants::LINEAR_SLOP,
            max_length: crate::constants::HUGE,
            enable_motor: false,
            max_motor_force: 0.0,
            motor_speed: 0.0,
            impulse: 0.0,
            lower_impulse: 0.0,
            upper_impulse: 0.0,
            motor_impulse: 0.0,
            axial_mass: 0.0,
            distance_softness: Softness::RIGID,
        }
    }
}

impl DistanceJoint {
    /// Computes the effective mass along the current axis for the step.
    pub(crate) fn prepare(&mut self, sim: &JointSim, ctx: &StepContext) {
        let r_a = sim.frame_a.position;
        let r_b = sim.frame_b.position;
        let separation = (r_b - r_a) + sim.delta_center;
        let axis = normalize(separation);
        let cr_a = r_a.cross(axis);
        let cr_b = r_b.cross(axis);
        let k = sim.inv_mass_a
            + sim.inv_mass_b
            + cr_a.dot(sim.inv_ia * cr_a)
            + cr_b.dot(sim.inv_ib * cr_b);
        self.axial_mass = if k > 0.0 { 1.0 / k } else { 0.0 };
        self.distance_softness = Softness::new(self.hertz, self.damping_ratio, ctx.h);
        if !ctx.enable_warm_starting {
            self.impulse = 0.0;
            self.lower_impulse = 0.0;
            self.upper_impulse = 0.0;
            self.motor_impulse = 0.0;
        }
    }

    /// Applies the accumulated impulses to the bodies.
    pub(crate) fn warm_start(&mut self, sim: &JointSim, states: &mut [BodyState]) {
        let mut state_a = load_state(states, sim.index_a);
        let mut state_b = load_state(states, sim.index_b);
        let r_a = state_a.delta_rotation * sim.frame_a.position;
        let r_b = state_b.delta_rotation * sim.frame_b.position;
        let ds = (state_b.delta_position - state_a.delta_position) + (r_b - r_a);
        let separation = sim.delta_center + ds;
        let axis = normalize(separation);
        let axial_impulse =
            self.impulse + self.lower_impulse - self.upper_impulse + self.motor_impulse;
        let p = axis * axial_impulse;
        if state_a.dynamic {
            state_a.linear_velocity = mul_sub(state_a.linear_velocity, sim.inv_mass_a, p);
            state_a.angular_velocity -= sim.inv_ia * r_a.cross(p);
        }
        if state_b.dynamic {
            state_b.linear_velocity = mul_add(state_b.linear_velocity, sim.inv_mass_b, p);
            state_b.angular_velocity += sim.inv_ib * r_b.cross(p);
        }
        store_state(states, sim.index_a, &state_a);
        store_state(states, sim.index_b, &state_b);
    }

    /// Solves the length constraint for one iteration.
    pub(crate) fn solve(
        &mut self,
        sim: &JointSim,
        states: &mut [BodyState],
        ctx: &StepContext,
        use_bias: bool,
    ) {
        let mut state_a = load_state(states, sim.index_a);
        let mut state_b = load_state(states, sim.index_b);
        let mut v_a = state_a.linear_velocity;
        let mut w_a = state_a.angular_velocity;
        let mut v_b = state_b.linear_velocity;
        let mut w_b = state_b.angular_velocity;
        let (m_a, m_b) = (sim.inv_mass_a, sim.inv_mass_b);
        let (i_a, i_b) = (sim.inv_ia, sim.inv_ib);
        let r_a = state_a.delta_rotation * sim.frame_a.position;
        let r_b = state_b.delta_rotation * sim.frame_b.position;
        let ds = (state_b.delta_position - state_a.delta_position) + (r_b - r_a);
        let separation = sim.delta_center + ds;
        let length = separation.length();
        let axis = normalize(separation);
        let soft = sim.constraint_softness;
        let relative_velocity = |v_a: Vec3, w_a: Vec3, v_b: Vec3, w_b: Vec3| {
            axis.dot((v_b - v_a) + (w_b.cross(r_b) - w_a.cross(r_a)))
        };
        let apply = |p: Vec3, v_a: &mut Vec3, w_a: &mut Vec3, v_b: &mut Vec3, w_b: &mut Vec3| {
            *v_a = mul_sub(*v_a, m_a, p);
            *w_a -= i_a * r_a.cross(p);
            *v_b = mul_add(*v_b, m_b, p);
            *w_b += i_b * r_b.cross(p);
        };

        if self.enable_spring && (self.min_length < self.max_length || !self.enable_limit) {
            if self.hertz > 0.0 {
                let cdot = relative_velocity(v_a, w_a, v_b, w_b);
                let c = length - self.length;
                let bias = self.distance_softness.bias_rate * c;
                let m = self.distance_softness.mass_scale * self.axial_mass;
                let old_impulse = self.impulse;
                let impulse =
                    -m * (cdot + bias) - self.distance_softness.impulse_scale * old_impulse;
                let h = ctx.h;
                self.impulse = (self.impulse + impulse)
                    .clamp(self.lower_spring_force * h, self.upper_spring_force * h);
                let impulse = self.impulse - old_impulse;
                apply(axis * impulse, &mut v_a, &mut w_a, &mut v_b, &mut w_b);
            }
            if self.enable_limit {
                {
                    let cdot = relative_velocity(v_a, w_a, v_b, w_b);
                    let c = length - self.min_length;
                    let (bias, mass_coeff, impulse_coeff) =
                        crate::joint::spherical::limit_terms(c, ctx, use_bias, &soft);
                    let impulse = -mass_coeff * self.axial_mass * (cdot + bias)
                        - impulse_coeff * self.lower_impulse;
                    let new_impulse = (self.lower_impulse + impulse).max(0.0);
                    let impulse = new_impulse - self.lower_impulse;
                    self.lower_impulse = new_impulse;
                    apply(axis * impulse, &mut v_a, &mut w_a, &mut v_b, &mut w_b);
                }
                {
                    let cdot = axis.dot((v_a - v_b) + (w_a.cross(r_a) - w_b.cross(r_b)));
                    let c = self.max_length - length;
                    let (bias, mass_scale, impulse_scale) =
                        crate::joint::spherical::limit_terms(c, ctx, use_bias, &soft);
                    let impulse = -mass_scale * self.axial_mass * (cdot + bias)
                        - impulse_scale * self.upper_impulse;
                    let new_impulse = (self.upper_impulse + impulse).max(0.0);
                    let impulse = new_impulse - self.upper_impulse;
                    self.upper_impulse = new_impulse;
                    apply(axis * -impulse, &mut v_a, &mut w_a, &mut v_b, &mut w_b);
                }
            }
            if self.enable_motor {
                let cdot = relative_velocity(v_a, w_a, v_b, w_b);
                let impulse = self.axial_mass * (self.motor_speed - cdot);
                let old_impulse = self.motor_impulse;
                let max_impulse = ctx.h * self.max_motor_force;
                self.motor_impulse =
                    (self.motor_impulse + impulse).clamp(-max_impulse, max_impulse);
                let impulse = self.motor_impulse - old_impulse;
                apply(axis * impulse, &mut v_a, &mut w_a, &mut v_b, &mut w_b);
            }
        } else {
            let cdot = relative_velocity(v_a, w_a, v_b, w_b);
            let c = length - self.length;
            let mut bias = 0.0;
            let mut mass_scale = 1.0;
            let mut impulse_scale = 0.0;
            if use_bias {
                bias = soft.bias_rate * c;
                mass_scale = soft.mass_scale;
                impulse_scale = soft.impulse_scale;
            }
            let impulse =
                -mass_scale * self.axial_mass * (cdot + bias) - impulse_scale * self.impulse;
            self.impulse += impulse;
            apply(axis * impulse, &mut v_a, &mut w_a, &mut v_b, &mut w_b);
        }

        state_a.linear_velocity = v_a;
        state_a.angular_velocity = w_a;
        state_b.linear_velocity = v_b;
        state_b.angular_velocity = w_b;
        store_state(states, sim.index_a, &state_a);
        store_state(states, sim.index_b, &state_b);
    }
}
