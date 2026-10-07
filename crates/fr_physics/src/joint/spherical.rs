//! The spherical joint: a point to point constraint with optional cone and twist limits, a
//! rotation spring towards a target and a motor.

use fr_math::{Mat3, Quat, Vec3};

use crate::joint::JointSim;
use crate::math::{
    Softness, delta_quat_to_rotation, dot_quat, inv_mul_quat, invert_matrix, mul_add, mul_quat,
    mul_sub, negate_quat, normalize, skew, solve3, swing_angle, twist_angle,
};
use crate::solver::{BodyState, StepContext, load_state, store_state};

/// A ball and socket joint. The cone axis is the z axis of the frame on body A and the twist
/// axis is the z axis of the frame on body B.
#[derive(Clone, Debug)]
pub struct SphericalJoint {
    /// Whether the swing of the twist axis away from the cone axis is limited.
    pub enable_cone_limit: bool,
    /// The largest swing angle in radians.
    pub cone_angle: f32,
    /// Whether the twist around the z axis is limited.
    pub enable_twist_limit: bool,
    /// The lowest twist angle in radians.
    pub lower_twist_angle: f32,
    /// The highest twist angle in radians.
    pub upper_twist_angle: f32,
    /// Whether a spring pulls the relative rotation towards `target_rotation`.
    pub enable_spring: bool,
    /// The stiffness of the spring in hertz.
    pub hertz: f32,
    /// The damping ratio of the spring.
    pub damping_ratio: f32,
    /// The relative rotation of frame B in frame A the spring pulls towards.
    pub target_rotation: Quat,
    /// Whether the motor drives the relative angular velocity.
    pub enable_motor: bool,
    /// The target relative angular velocity of the motor, in world axes.
    pub motor_velocity: Vec3,
    /// The largest torque of the motor, which is joint friction at zero velocity.
    pub max_motor_torque: f32,
    /// The impulse of the point constraint over the last sub-step.
    pub(crate) linear_impulse: Vec3,
    /// The impulse of the spring.
    pub(crate) spring_impulse: Vec3,
    /// The impulse of the motor.
    pub(crate) motor_impulse: Vec3,
    /// The impulse of the cone limit.
    pub(crate) swing_impulse: f32,
    /// The impulse of the lower twist limit.
    pub(crate) lower_twist_impulse: f32,
    /// The impulse of the upper twist limit.
    pub(crate) upper_twist_impulse: f32,
    /// The axis the cone limit acts about.
    pub(crate) swing_axis: Vec3,
    /// The effective mass of the cone limit.
    pub(crate) swing_mass: f32,
    /// The jacobian of the twist limit.
    pub(crate) twist_jacobian: Vec3,
    /// The effective mass of the twist limit.
    pub(crate) twist_mass: f32,
    /// The effective mass of the three axis rotation constraints.
    pub(crate) rotation_mass: Mat3,
    /// The softness of the spring for this step.
    pub(crate) spring_softness: Softness,
}

impl Default for SphericalJoint {
    /// A free ball joint with no limits, spring or motor.
    fn default() -> Self {
        Self {
            enable_cone_limit: false,
            cone_angle: 0.5 * std::f32::consts::PI,
            enable_twist_limit: false,
            lower_twist_angle: 0.0,
            upper_twist_angle: 0.0,
            enable_spring: false,
            hertz: 0.0,
            damping_ratio: 0.0,
            target_rotation: Quat::IDENTITY,
            enable_motor: false,
            motor_velocity: Vec3::ZERO,
            max_motor_torque: 0.0,
            linear_impulse: Vec3::ZERO,
            spring_impulse: Vec3::ZERO,
            motor_impulse: Vec3::ZERO,
            swing_impulse: 0.0,
            lower_twist_impulse: 0.0,
            upper_twist_impulse: 0.0,
            swing_axis: Vec3::ZERO,
            swing_mass: 0.0,
            twist_jacobian: Vec3::ZERO,
            twist_mass: 0.0,
            rotation_mass: Mat3::ZERO,
            spring_softness: Softness::RIGID,
        }
    }
}

impl SphericalJoint {
    /// Computes the effective masses and axes for the step.
    pub(crate) fn prepare(&mut self, sim: &JointSim, ctx: &StepContext) {
        let inv_inertia_sum = sim.inv_ia + sim.inv_ib;
        let cone_axis = sim.frame_a.rotation * Vec3::Z;
        let twist_axis = sim.frame_b.rotation * Vec3::Z;
        if self.enable_cone_limit {
            let swing_axis = normalize(cone_axis.cross(twist_axis));
            let k = swing_axis.dot(inv_inertia_sum * swing_axis);
            self.swing_mass = if k > 0.0 { 1.0 / k } else { 0.0 };
            self.swing_axis = swing_axis;
        }
        if self.enable_twist_limit {
            let rel_q = inv_mul_quat(sim.frame_a.rotation, sim.frame_b.rotation);
            let tan_theta_over2 = ((rel_q.x * rel_q.x + rel_q.y * rel_q.y)
                / (rel_q.z * rel_q.z + rel_q.w * rel_q.w))
                .sqrt();
            let swing_axis = normalize(cone_axis.cross(twist_axis));
            let perp_axis = swing_axis.cross(cone_axis);
            let twist_jacobian = mul_add(cone_axis, tan_theta_over2, perp_axis);
            let k = twist_jacobian.dot(inv_inertia_sum * twist_jacobian);
            self.twist_mass = if k > 0.0 { 1.0 / k } else { 0.0 };
            self.twist_jacobian = twist_jacobian;
        }
        self.rotation_mass = if sim.fixed_rotation {
            Mat3::ZERO
        } else {
            invert_matrix(inv_inertia_sum)
        };
        self.spring_softness = Softness::new(self.hertz, self.damping_ratio, ctx.h);
        if !ctx.enable_warm_starting {
            self.linear_impulse = Vec3::ZERO;
            self.motor_impulse = Vec3::ZERO;
            self.spring_impulse = Vec3::ZERO;
            self.swing_impulse = 0.0;
            self.lower_twist_impulse = 0.0;
            self.upper_twist_impulse = 0.0;
        }
    }

    /// Applies the accumulated impulses to the bodies.
    pub(crate) fn warm_start(&mut self, sim: &JointSim, states: &mut [BodyState]) {
        let mut state_a = load_state(states, sim.index_a);
        let mut state_b = load_state(states, sim.index_b);
        let r_a = state_a.delta_rotation * sim.frame_a.position;
        let r_b = state_b.delta_rotation * sim.frame_b.position;
        let mut angular_impulse = self.spring_impulse + self.motor_impulse;
        angular_impulse = mul_sub(angular_impulse, self.swing_impulse, self.swing_axis);
        angular_impulse = mul_add(
            angular_impulse,
            self.lower_twist_impulse - self.upper_twist_impulse,
            self.twist_jacobian,
        );
        state_a.linear_velocity =
            mul_sub(state_a.linear_velocity, sim.inv_mass_a, self.linear_impulse);
        state_a.angular_velocity -= sim.inv_ia * (r_a.cross(self.linear_impulse) + angular_impulse);
        state_b.linear_velocity =
            mul_add(state_b.linear_velocity, sim.inv_mass_b, self.linear_impulse);
        state_b.angular_velocity += sim.inv_ib * (r_b.cross(self.linear_impulse) + angular_impulse);
        store_state(states, sim.index_a, &state_a);
        store_state(states, sim.index_b, &state_b);
    }

    /// Solves the spring, motor, limits and point constraint for one iteration.
    pub(crate) fn solve(
        &mut self,
        sim: &JointSim,
        states: &mut [BodyState],
        ctx: &StepContext,
        use_bias: bool,
    ) {
        let mut state_a = load_state(states, sim.index_a);
        let mut state_b = load_state(states, sim.index_b);
        let mut w_a = state_a.angular_velocity;
        let mut w_b = state_b.angular_velocity;
        let mut v_a = state_a.linear_velocity;
        let mut v_b = state_b.linear_velocity;
        let (i_a, i_b) = (sim.inv_ia, sim.inv_ib);
        let fixed_rotation = sim.fixed_rotation;
        let quat_a = mul_quat(state_a.delta_rotation, sim.frame_a.rotation);
        let quat_b = mul_quat(state_b.delta_rotation, sim.frame_b.rotation);
        let rel_q = inv_mul_quat(quat_a, quat_b);
        let soft = sim.constraint_softness;

        if self.enable_spring && !fixed_rotation {
            let delta_rotation = delta_quat_to_rotation(rel_q, self.target_rotation);
            let c = -(quat_a * delta_rotation);
            let bias = c * self.spring_softness.bias_rate;
            let mass_scale = self.spring_softness.mass_scale;
            let impulse_scale = self.spring_softness.impulse_scale;
            let cdot = w_b - w_a;
            let impulse = mul_sub(
                (self.rotation_mass * (cdot + bias)) * -mass_scale,
                impulse_scale,
                self.spring_impulse,
            );
            self.spring_impulse += impulse;
            w_a -= i_a * impulse;
            w_b += i_b * impulse;
        }

        if self.enable_motor && !fixed_rotation {
            let cdot = w_b - w_a;
            let mut lambda = -(self.rotation_mass * (cdot - self.motor_velocity));
            let mut new_impulse = self.motor_impulse + lambda;
            let length = new_impulse.length();
            let max_impulse = self.max_motor_torque * ctx.h;
            if length > max_impulse {
                new_impulse *= max_impulse / length;
            }
            lambda = new_impulse - self.motor_impulse;
            self.motor_impulse = new_impulse;
            w_a -= i_a * lambda;
            w_b += i_b * lambda;
        }

        if self.enable_twist_limit && !fixed_rotation {
            let twist = twist_angle(rel_q);
            let twist_jacobian = self.twist_jacobian;
            {
                let c = twist - self.lower_twist_angle;
                let (bias, mass_scale, impulse_scale) = limit_terms(c, ctx, use_bias, &soft);
                let cdot = (w_b - w_a).dot(twist_jacobian);
                let old_impulse = self.lower_twist_impulse;
                let delta =
                    -mass_scale * self.twist_mass * (cdot + bias) - impulse_scale * old_impulse;
                self.lower_twist_impulse = (old_impulse + delta).max(0.0);
                let delta = self.lower_twist_impulse - old_impulse;
                w_a = mul_sub(w_a, delta, i_a * twist_jacobian);
                w_b = mul_add(w_b, delta, i_b * twist_jacobian);
            }
            {
                let c = self.upper_twist_angle - twist;
                let (bias, mass_scale, impulse_scale) = limit_terms(c, ctx, use_bias, &soft);
                let cdot = (w_a - w_b).dot(twist_jacobian);
                let old_impulse = self.upper_twist_impulse;
                let delta =
                    -mass_scale * self.twist_mass * (cdot + bias) - impulse_scale * old_impulse;
                self.upper_twist_impulse = (old_impulse + delta).max(0.0);
                let delta = self.upper_twist_impulse - old_impulse;
                w_a = mul_add(w_a, delta, i_a * twist_jacobian);
                w_b = mul_sub(w_b, delta, i_b * twist_jacobian);
            }
        }

        if self.enable_cone_limit && !fixed_rotation {
            let swing = swing_angle(rel_q);
            let swing_axis = self.swing_axis;
            let c = self.cone_angle - swing;
            let (bias, mass_scale, impulse_scale) = limit_terms(c, ctx, use_bias, &soft);
            let cdot = (w_a - w_b).dot(swing_axis);
            let old_impulse = self.swing_impulse;
            let delta = -mass_scale * self.swing_mass * (cdot + bias) - impulse_scale * old_impulse;
            self.swing_impulse = (old_impulse + delta).max(0.0);
            let delta = self.swing_impulse - old_impulse;
            w_a = mul_add(w_a, delta, i_a * swing_axis);
            w_b = mul_sub(w_b, delta, i_b * swing_axis);
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

    /// The relative rotation of frame B in frame A, for reading the angles of the joint.
    pub fn relative_rotation(rotation_a: Quat, rotation_b: Quat) -> Quat {
        let quat_b = if dot_quat(rotation_a, rotation_b) < 0.0 {
            negate_quat(rotation_b)
        } else {
            rotation_b
        };
        inv_mul_quat(rotation_a, quat_b)
    }
}

/// The bias, mass scale and impulse scale of a one sided limit with constraint value `c`: a
/// speculative bias while the limit is open, soft while it is violated and a bias is wanted.
pub(crate) fn limit_terms(
    c: f32,
    ctx: &StepContext,
    use_bias: bool,
    soft: &Softness,
) -> (f32, f32, f32) {
    if c > 0.0 {
        (c * ctx.inv_h, 1.0, 0.0)
    } else if use_bias {
        (soft.bias_rate * c, soft.mass_scale, soft.impulse_scale)
    } else {
        (0.0, 1.0, 0.0)
    }
}

/// The effective mass matrix inverse of the point to point constraint.
pub(crate) fn point_constraint_matrix(r_a: Vec3, r_b: Vec3, sim: &JointSim) -> Mat3 {
    let s_a = skew(r_a);
    let s_b = skew(r_b);
    let k_a = s_a * (sim.inv_ia * s_a);
    let k_b = s_b * (sim.inv_ib * s_b);
    let mut k = -(k_a + k_b);
    let m = sim.inv_mass_a + sim.inv_mass_b;
    k.x_axis.x += m;
    k.y_axis.y += m;
    k.z_axis.z += m;
    k
}
