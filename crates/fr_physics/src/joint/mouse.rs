//! The mouse joint: a soft point constraint that pulls a point of one body towards a target in
//! the world, with a spring and a force limit, for dragging bodies around.

use fr_math::Vec3;

use crate::joint::JointSim;
use crate::joint::spherical::point_constraint_matrix;
use crate::math::{Softness, mul_add, mul_sub, solve3};
use crate::solver::{BodyState, StepContext, load_state, store_state};

/// A spring from the anchor of frame B to a world `target`. Body A is only the other end of the
/// joint and should be static; the joint acts on body B alone.
#[derive(Clone, Debug)]
pub struct MouseJoint {
    /// The point in the world the anchor is pulled towards.
    pub target: Vec3,
    /// The stiffness of the spring in hertz.
    pub hertz: f32,
    /// The damping ratio of the spring.
    pub damping_ratio: f32,
    /// The largest force the spring may apply.
    pub max_force: f32,
    /// The impulse of the constraint over the last sub-step.
    pub(crate) linear_impulse: Vec3,
    /// The softness of the spring for this step.
    pub(crate) linear_softness: Softness,
}

impl Default for MouseJoint {
    /// A joint with Box2D's testbed spring, five hertz at a damping ratio of 0.7, and no force
    /// limit.
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            hertz: 5.0,
            damping_ratio: 0.7,
            max_force: f32::MAX,
            linear_impulse: Vec3::ZERO,
            linear_softness: Softness::RIGID,
        }
    }
}

impl MouseJoint {
    /// A joint with a spring of `hertz` and `damping_ratio` that applies at most `max_force`;
    /// the target is set when the joint is created.
    pub fn new(hertz: f32, damping_ratio: f32, max_force: f32) -> Self {
        Self {
            hertz,
            damping_ratio,
            max_force,
            ..Self::default()
        }
    }

    /// Computes the softness of the spring for the step.
    pub(crate) fn prepare(&mut self, _sim: &JointSim, ctx: &StepContext) {
        self.linear_softness = Softness::new(self.hertz, self.damping_ratio, ctx.h);
        if !ctx.enable_warm_starting {
            self.linear_impulse = Vec3::ZERO;
        }
    }

    /// Applies the accumulated impulse to body B.
    pub(crate) fn warm_start(&mut self, sim: &JointSim, states: &mut [BodyState]) {
        let mut state_b = load_state(states, sim.index_b);
        let r_b = state_b.delta_rotation * sim.frame_b.position;
        state_b.linear_velocity =
            mul_add(state_b.linear_velocity, sim.inv_mass_b, self.linear_impulse);
        state_b.angular_velocity += sim.inv_ib * r_b.cross(self.linear_impulse);
        store_state(states, sim.index_b, &state_b);
    }

    /// Solves the spring for one iteration; the spring bias is applied in the relax iterations
    /// too, since it is a spring and not a position correction.
    pub(crate) fn solve(&mut self, sim: &JointSim, states: &mut [BodyState], ctx: &StepContext) {
        let mut state_b = load_state(states, sim.index_b);
        let r_b = state_b.delta_rotation * sim.frame_b.position;
        let cdot = state_b.linear_velocity + state_b.angular_velocity.cross(r_b);
        let anchor = ((sim.center_b + state_b.delta_position) + r_b) - self.target;
        let bias = anchor * self.linear_softness.bias_rate;
        let k = point_constraint_matrix(Vec3::ZERO, r_b, sim);
        let b = solve3(k, cdot + bias);
        let mut impulse = mul_sub(
            b * -self.linear_softness.mass_scale,
            self.linear_softness.impulse_scale,
            self.linear_impulse,
        );
        let mut new_impulse = self.linear_impulse + impulse;
        let max_impulse = self.max_force * ctx.h;
        let length = new_impulse.length();
        if length > max_impulse {
            new_impulse *= max_impulse / length;
        }
        impulse = new_impulse - self.linear_impulse;
        self.linear_impulse = new_impulse;
        state_b.linear_velocity = mul_add(state_b.linear_velocity, sim.inv_mass_b, impulse);
        state_b.angular_velocity += sim.inv_ib * r_b.cross(impulse);
        store_state(states, sim.index_b, &state_b);
    }
}
