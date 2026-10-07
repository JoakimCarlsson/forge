//! The revolute joint: a hinge about the z axis of the joint frame with an optional angle
//! limit, torque limited spring and motor.

use fr_math::Vec3;

use crate::joint::JointSim;
use crate::joint::spherical::{limit_terms, point_constraint_matrix};
use crate::math::{
    Softness, dot_quat, inv_mul_quat, invert2, mul_add, mul_quat, mul_sub, negate_quat, solve3,
    twist_angle,
};
use crate::solver::{BodyState, StepContext, load_state, store_state};

/// A hinge. The hinge axis is the z axis of the frame on body A and the angle is the rotation
/// of frame B in frame A about it, counter-clockwise positive.
#[derive(Clone, Debug)]
pub struct RevoluteJoint {
    /// Whether the angle is limited.
    pub enable_limit: bool,
    /// The lowest angle in radians.
    pub lower_angle: f32,
    /// The highest angle in radians.
    pub upper_angle: f32,
    /// Whether a spring pulls the angle towards `target_angle`.
    pub enable_spring: bool,
    /// The stiffness of the spring in hertz.
    pub hertz: f32,
    /// The damping ratio of the spring.
    pub damping_ratio: f32,
    /// The largest torque of the spring; unlimited by default.
    pub max_spring_torque: f32,
    /// The angle the spring pulls towards.
    pub target_angle: f32,
    /// Whether the motor drives the angular speed.
    pub enable_motor: bool,
    /// The target angular speed of the motor in radians per second.
    pub motor_speed: f32,
    /// The largest torque of the motor, which is joint friction at zero speed.
    pub max_motor_torque: f32,
    /// The impulse of the point constraint.
    pub(crate) linear_impulse: Vec3,
    /// The impulses of the two perpendicular rotation constraints.
    pub(crate) perp_impulse: (f32, f32),
    /// The impulse of the spring.
    pub(crate) spring_impulse: f32,
    /// The impulse of the motor.
    pub(crate) motor_impulse: f32,
    /// The impulse of the lower limit.
    pub(crate) lower_impulse: f32,
    /// The impulse of the upper limit.
    pub(crate) upper_impulse: f32,
    /// The hinge axis in the world.
    pub(crate) rotation_axis_z: Vec3,
    /// The first perpendicular constraint axis.
    pub(crate) perp_axis_x: Vec3,
    /// The second perpendicular constraint axis.
    pub(crate) perp_axis_y: Vec3,
    /// The effective mass about the hinge axis.
    pub(crate) axial_mass: f32,
    /// The softness of the spring for this step.
    pub(crate) spring_softness: Softness,
}

impl Default for RevoluteJoint {
    /// A free hinge with no limit, spring or motor.
    fn default() -> Self {
        Self {
            enable_limit: false,
            lower_angle: 0.0,
            upper_angle: 0.0,
            enable_spring: false,
            hertz: 0.0,
            damping_ratio: 0.0,
            max_spring_torque: f32::MAX,
            target_angle: 0.0,
            enable_motor: false,
            motor_speed: 0.0,
            max_motor_torque: 0.0,
            linear_impulse: Vec3::ZERO,
            perp_impulse: (0.0, 0.0),
            spring_impulse: 0.0,
            motor_impulse: 0.0,
            lower_impulse: 0.0,
            upper_impulse: 0.0,
            rotation_axis_z: Vec3::Z,
            perp_axis_x: Vec3::X,
            perp_axis_y: Vec3::Y,
            axial_mass: 0.0,
            spring_softness: Softness::RIGID,
        }
    }
}

/// The two axes perpendicular to the hinge, from the relative rotation `rel_q` seen in `frame_q`.
fn perp_axes(frame_q: fr_math::Quat, rel_q: fr_math::Quat) -> (Vec3, Vec3) {
    let v = Vec3::new(rel_q.x, rel_q.y, rel_q.z);
    let axis_x = (frame_q * (Vec3::X * rel_q.w + v.cross(Vec3::X))) * 0.5;
    let axis_y = (frame_q * (Vec3::Y * rel_q.w + v.cross(Vec3::Y))) * 0.5;
    (axis_x, axis_y)
}

impl RevoluteJoint {
    /// Computes the axes and effective masses for the step.
    pub(crate) fn prepare(&mut self, sim: &JointSim, ctx: &StepContext) {
        let inv_inertia_sum = sim.inv_ia + sim.inv_ib;
        let axis = sim.frame_a.rotation * Vec3::Z;
        let k = axis.dot(inv_inertia_sum * axis);
        self.axial_mass = if k > 0.0 { 1.0 / k } else { 0.0 };
        self.rotation_axis_z = axis;
        let rel_q = inv_mul_quat(sim.frame_a.rotation, sim.frame_b.rotation);
        let (axis_x, axis_y) = perp_axes(sim.frame_a.rotation, rel_q);
        self.perp_axis_x = axis_x;
        self.perp_axis_y = axis_y;
        self.spring_softness = Softness::new(self.hertz, self.damping_ratio, ctx.h);
        if !ctx.enable_warm_starting {
            self.linear_impulse = Vec3::ZERO;
            self.perp_impulse = (0.0, 0.0);
            self.motor_impulse = 0.0;
            self.spring_impulse = 0.0;
            self.lower_impulse = 0.0;
            self.upper_impulse = 0.0;
        }
    }

    /// Applies the accumulated impulses to the bodies.
    pub(crate) fn warm_start(&mut self, sim: &JointSim, states: &mut [BodyState]) {
        let mut state_a = load_state(states, sim.index_a);
        let mut state_b = load_state(states, sim.index_b);
        let r_a = state_a.delta_rotation * sim.frame_a.position;
        let r_b = state_b.delta_rotation * sim.frame_b.position;
        let axial_impulse =
            self.spring_impulse + self.motor_impulse + self.lower_impulse - self.upper_impulse;
        let mut angular_impulse =
            (self.perp_axis_x * self.perp_impulse.0) + (self.perp_axis_y * self.perp_impulse.1);
        angular_impulse = mul_add(angular_impulse, axial_impulse, self.rotation_axis_z);
        state_a.linear_velocity =
            mul_sub(state_a.linear_velocity, sim.inv_mass_a, self.linear_impulse);
        state_a.angular_velocity -= sim.inv_ia * (r_a.cross(self.linear_impulse) + angular_impulse);
        state_b.linear_velocity =
            mul_add(state_b.linear_velocity, sim.inv_mass_b, self.linear_impulse);
        state_b.angular_velocity += sim.inv_ib * (r_b.cross(self.linear_impulse) + angular_impulse);
        store_state(states, sim.index_a, &state_a);
        store_state(states, sim.index_b, &state_b);
    }

    /// Solves the spring, motor, limit, perpendicular and point constraints for one iteration.
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
        let (i_a, i_b) = (sim.inv_ia, sim.inv_ib);
        let fixed_rotation = sim.fixed_rotation;
        let quat_a = mul_quat(state_a.delta_rotation, sim.frame_a.rotation);
        let mut quat_b = mul_quat(state_b.delta_rotation, sim.frame_b.rotation);
        if dot_quat(quat_a, quat_b) < 0.0 {
            quat_b = negate_quat(quat_b);
        }
        let rel_q = inv_mul_quat(quat_a, quat_b);
        let soft = sim.constraint_softness;
        let axis = self.rotation_axis_z;

        if self.enable_spring && !fixed_rotation {
            let angle = twist_angle(rel_q);
            let c = angle - self.target_angle;
            let bias = self.spring_softness.bias_rate * c;
            let mass_scale = self.spring_softness.mass_scale;
            let impulse_scale = self.spring_softness.impulse_scale;
            let cdot = (w_b - w_a).dot(axis);
            let delta =
                -mass_scale * self.axial_mass * (cdot + bias) - impulse_scale * self.spring_impulse;
            let max_impulse = self.max_spring_torque * ctx.h;
            let new_impulse = (self.spring_impulse + delta).clamp(-max_impulse, max_impulse);
            let delta = new_impulse - self.spring_impulse;
            self.spring_impulse = new_impulse;
            w_a = mul_sub(w_a, delta, i_a * axis);
            w_b = mul_add(w_b, delta, i_b * axis);
        }

        if self.enable_motor && !fixed_rotation {
            let cdot = (w_b - w_a).dot(axis) - self.motor_speed;
            let delta = -self.axial_mass * cdot;
            let max_impulse = self.max_motor_torque * ctx.h;
            let new_impulse = (self.motor_impulse + delta).clamp(-max_impulse, max_impulse);
            let delta = new_impulse - self.motor_impulse;
            self.motor_impulse = new_impulse;
            w_a = mul_sub(w_a, delta, i_a * axis);
            w_b = mul_add(w_b, delta, i_b * axis);
        }

        if self.enable_limit && !fixed_rotation {
            let angle = twist_angle(rel_q);
            {
                let c = angle - self.lower_angle;
                let (bias, mass_scale, impulse_scale) = limit_terms(c, ctx, use_bias, &soft);
                let cdot = (w_b - w_a).dot(axis);
                let old_impulse = self.lower_impulse;
                let delta =
                    -mass_scale * self.axial_mass * (cdot + bias) - impulse_scale * old_impulse;
                self.lower_impulse = (old_impulse + delta).max(0.0);
                let delta = self.lower_impulse - old_impulse;
                w_a = mul_sub(w_a, delta, i_a * axis);
                w_b = mul_add(w_b, delta, i_b * axis);
            }
            {
                let c = self.upper_angle - angle;
                let (bias, mass_scale, impulse_scale) = limit_terms(c, ctx, use_bias, &soft);
                let cdot = (w_a - w_b).dot(axis);
                let old_impulse = self.upper_impulse;
                let delta =
                    -mass_scale * self.axial_mass * (cdot + bias) - impulse_scale * old_impulse;
                self.upper_impulse = (old_impulse + delta).max(0.0);
                let delta = self.upper_impulse - old_impulse;
                w_a = mul_add(w_a, delta, i_a * axis);
                w_b = mul_sub(w_b, delta, i_b * axis);
            }
        }

        if !fixed_rotation {
            let mut bias = (0.0, 0.0);
            let mut mass_scale = 1.0;
            let mut impulse_scale = 0.0;
            if use_bias {
                bias = (soft.bias_rate * rel_q.x, soft.bias_rate * rel_q.y);
                mass_scale = soft.mass_scale;
                impulse_scale = soft.impulse_scale;
            }
            let (perp_x, perp_y) = perp_axes(quat_a, rel_q);
            self.perp_axis_x = perp_x;
            self.perp_axis_y = perp_y;
            let inv_inertia_sum = i_a + i_b;
            let kxx = perp_x.dot(inv_inertia_sum * perp_x);
            let kyy = perp_y.dot(inv_inertia_sum * perp_y);
            let kxy = perp_x.dot(inv_inertia_sum * perp_y);
            let w_rel = w_b - w_a;
            let cdot = (w_rel.dot(perp_x), w_rel.dot(perp_y));
            let old_impulse = self.perp_impulse;
            let rhs = (cdot.0 + bias.0, cdot.1 + bias.1);
            let (inv_xx, inv_xy, inv_yy) = invert2(kxx, kxy, kyy);
            let sol = (
                inv_xx * rhs.0 + inv_xy * rhs.1,
                inv_xy * rhs.0 + inv_yy * rhs.1,
            );
            let delta = (
                -mass_scale * sol.0 - impulse_scale * old_impulse.0,
                -mass_scale * sol.1 - impulse_scale * old_impulse.1,
            );
            self.perp_impulse = (self.perp_impulse.0 + delta.0, self.perp_impulse.1 + delta.1);
            let angular_impulse = (perp_x * delta.0) + (perp_y * delta.1);
            w_a -= i_a * angular_impulse;
            w_b += i_b * angular_impulse;
        }

        {
            let r_a = state_a.delta_rotation * sim.frame_a.position;
            let r_b = state_b.delta_rotation * sim.frame_b.position;
            let cdot = ((v_b + w_b.cross(r_b)) - v_a) - w_a.cross(r_a);
            let mut bias = Vec3::ZERO;
            let mut mass_scale = 1.0;
            let mut impulse_scale = 0.0;
            if use_bias {
                let dc_a = state_a.delta_position;
                let dc_b = state_b.delta_position;
                let separation = ((dc_b - dc_a) + (r_b - r_a)) + sim.delta_center;
                bias = separation * soft.bias_rate;
                mass_scale = soft.mass_scale;
                impulse_scale = soft.impulse_scale;
            }
            let k = point_constraint_matrix(r_a, r_b, sim);
            let b = solve3(k, cdot + bias);
            let impulse = (b * -mass_scale) - (self.linear_impulse * impulse_scale);
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
