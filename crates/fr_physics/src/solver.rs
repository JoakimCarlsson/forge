//! Per step solver data shared by contacts and joints: the body state of a sub-step and the
//! step context.

use fr_core::{Quat, Vec3};

use crate::math::Softness;

/// The motion of one awake body within a step.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BodyState {
    /// The linear velocity of the centre of mass.
    pub(crate) linear_velocity: Vec3,
    /// The angular velocity.
    pub(crate) angular_velocity: Vec3,
    /// How far the centre of mass has moved since the start of the step.
    pub(crate) delta_position: Vec3,
    /// How far the body has turned since the start of the step.
    pub(crate) delta_rotation: Quat,
    /// Whether constraints may change the velocity; false for kinematic bodies and the dummy.
    pub(crate) dynamic: bool,
}

impl BodyState {
    /// The state of a body that never moves, used for static bodies.
    pub(crate) const DUMMY: Self = Self {
        linear_velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        delta_position: Vec3::ZERO,
        delta_rotation: Quat::IDENTITY,
        dynamic: false,
    };
}

/// The time and tuning values of the step being solved.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StepContext {
    /// The length of the step.
    pub(crate) dt: f32,
    /// The inverse of the step length.
    pub(crate) inv_dt: f32,
    /// The length of a sub-step.
    pub(crate) h: f32,
    /// The inverse of the sub-step length.
    pub(crate) inv_h: f32,
    /// The softness of contacts between dynamic bodies.
    pub(crate) contact_softness: Softness,
    /// The softness of contacts with static bodies.
    pub(crate) static_softness: Softness,
    /// The approach speed below which restitution is ignored.
    pub(crate) restitution_threshold: f32,
    /// The largest speed contacts push bodies apart at.
    pub(crate) contact_speed: f32,
    /// The largest linear speed of a body.
    pub(crate) max_linear_speed: f32,
    /// Whether impulses of the previous step are applied before solving.
    pub(crate) enable_warm_starting: bool,
}

/// The state at `index`, or the dummy state of a body that does not move.
pub(crate) fn load_state(states: &[BodyState], index: usize) -> BodyState {
    states.get(index).copied().unwrap_or(BodyState::DUMMY)
}

/// Writes back the velocities of `state` when it is dynamic.
pub(crate) fn store_state(states: &mut [BodyState], index: usize, state: &BodyState) {
    if !state.dynamic {
        return;
    }
    if let Some(target) = states.get_mut(index) {
        target.linear_velocity = state.linear_velocity;
        target.angular_velocity = state.angular_velocity;
    }
}
