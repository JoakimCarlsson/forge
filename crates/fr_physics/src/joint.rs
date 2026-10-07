//! Joints between two bodies: spherical with cone and twist limits, revolute with angle limits,
//! weld, distance and filter joints, solved in the soft step loop with the contacts.
//!
//! The local frame of a joint on each body places its anchor and its axes: the z axis of the
//! frame on body A is the cone axis of a spherical joint and the hinge axis of a revolute joint.

mod distance;
mod revolute;
mod spherical;
mod weld;

use fr_math::{Mat3, Vec3};

use crate::body::{Body, BodyId, BodyType};
use crate::island::IslandId;
use crate::math::{Pose, Softness, det};
use crate::slot::Handle;
use crate::solver::{BodyState, StepContext};

pub use distance::DistanceJoint;
pub use revolute::RevoluteJoint;
pub use spherical::SphericalJoint;
pub use weld::WeldJoint;

/// Marks [`JointId`].
#[derive(Clone, Copy, Debug)]
pub struct JointTag;

/// A handle to a joint of a world.
pub type JointId = Handle<JointTag>;

/// The type of a joint with its settings.
#[derive(Clone, Debug)]
pub enum JointKind {
    /// A ball and socket joint with optional cone and twist limits, spring and motor.
    Spherical(SphericalJoint),
    /// A hinge with an optional angle limit, spring and motor.
    Revolute(RevoluteJoint),
    /// Locks the two frames together, optionally softly.
    Weld(WeldJoint),
    /// Keeps two anchors at a distance, with optional spring, limits and motor.
    Distance(DistanceJoint),
    /// Does nothing but disable collision between the two bodies and keep them in one island.
    Filter,
}

/// The description of a joint to create.
#[derive(Clone, Debug)]
pub struct JointDef {
    /// The first body.
    pub body_a: BodyId,
    /// The second body.
    pub body_b: BodyId,
    /// The joint frame in the frame of body A.
    pub local_frame_a: Pose,
    /// The joint frame in the frame of body B.
    pub local_frame_b: Pose,
    /// Whether the connected bodies collide with each other.
    pub collide_connected: bool,
    /// The stiffness of the joint constraint, in hertz.
    pub constraint_hertz: f32,
    /// The damping ratio of the joint constraint.
    pub constraint_damping_ratio: f32,
    /// The type of the joint with its settings.
    pub kind: JointKind,
}

impl JointDef {
    /// A joint of `kind` between two bodies at the given local frames.
    pub fn new(
        body_a: BodyId,
        body_b: BodyId,
        local_frame_a: Pose,
        local_frame_b: Pose,
        kind: JointKind,
    ) -> Self {
        Self {
            body_a,
            body_b,
            local_frame_a,
            local_frame_b,
            collide_connected: false,
            constraint_hertz: 60.0,
            constraint_damping_ratio: 2.0,
            kind,
        }
    }
}

/// The data of both bodies a joint reads when it is prepared for a step.
#[derive(Clone, Copy, Debug)]
pub(crate) struct JointSim {
    /// The solver index of body A, or the null index when it does not move.
    pub(crate) index_a: usize,
    /// The solver index of body B, or the null index when it does not move.
    pub(crate) index_b: usize,
    /// The inverse mass of A.
    pub(crate) inv_mass_a: f32,
    /// The inverse mass of B.
    pub(crate) inv_mass_b: f32,
    /// The world inverse inertia of A.
    pub(crate) inv_ia: Mat3,
    /// The world inverse inertia of B.
    pub(crate) inv_ib: Mat3,
    /// Whether neither body can rotate.
    pub(crate) fixed_rotation: bool,
    /// The softness of the joint constraint for this step.
    pub(crate) constraint_softness: Softness,
    /// The frame on A with world rotation and position relative to the centre of mass.
    pub(crate) frame_a: Pose,
    /// The frame on B with world rotation and position relative to the centre of mass.
    pub(crate) frame_b: Pose,
    /// The vector from the centre of mass of A to that of B.
    pub(crate) delta_center: Vec3,
}

impl JointSim {
    /// The prepared data of a joint that is not yet in a step.
    const EMPTY: Self = Self {
        index_a: usize::MAX,
        index_b: usize::MAX,
        inv_mass_a: 0.0,
        inv_mass_b: 0.0,
        inv_ia: Mat3::ZERO,
        inv_ib: Mat3::ZERO,
        fixed_rotation: true,
        constraint_softness: Softness::RIGID,
        frame_a: Pose::IDENTITY,
        frame_b: Pose::IDENTITY,
        delta_center: Vec3::ZERO,
    };
}

/// A joint of a world.
#[derive(Clone, Debug)]
pub(crate) struct Joint {
    /// The first body.
    pub(crate) body_a: BodyId,
    /// The second body.
    pub(crate) body_b: BodyId,
    /// The joint frame in the frame of body A.
    pub(crate) local_frame_a: Pose,
    /// The joint frame in the frame of body B.
    pub(crate) local_frame_b: Pose,
    /// Whether the connected bodies collide.
    pub(crate) collide_connected: bool,
    /// The stiffness of the constraint.
    pub(crate) constraint_hertz: f32,
    /// The damping ratio of the constraint.
    pub(crate) constraint_damping_ratio: f32,
    /// The type and settings.
    pub(crate) kind: JointKind,
    /// The island the joint links.
    pub(crate) island: IslandId,
    /// The position in the island's joint list.
    pub(crate) island_index: usize,
    /// The data prepared for the current step.
    pub(crate) sim: JointSim,
}

impl Joint {
    /// A joint from its description.
    pub(crate) fn new(def: JointDef) -> Self {
        Self {
            body_a: def.body_a,
            body_b: def.body_b,
            local_frame_a: def.local_frame_a,
            local_frame_b: def.local_frame_b,
            collide_connected: def.collide_connected,
            constraint_hertz: def.constraint_hertz,
            constraint_damping_ratio: def.constraint_damping_ratio,
            kind: def.kind,
            island: IslandId::NULL,
            island_index: usize::MAX,
            sim: JointSim::EMPTY,
        }
    }

    /// Reads the bodies and sets up the joint for a step.
    pub(crate) fn prepare(&mut self, body_a: &Body, body_b: &Body, ctx: &StepContext) {
        let hertz = self.constraint_hertz.min(0.25 * ctx.inv_h);
        let sim = &mut self.sim;
        sim.constraint_softness = Softness::new(hertz, self.constraint_damping_ratio, ctx.h);
        sim.index_a = if body_a.is_awake() {
            body_a.solver_index
        } else {
            usize::MAX
        };
        sim.index_b = if body_b.is_awake() {
            body_b.solver_index
        } else {
            usize::MAX
        };
        sim.inv_mass_a = body_a.inv_mass;
        sim.inv_mass_b = body_b.inv_mass;
        sim.inv_ia = body_a.inv_inertia_world;
        sim.inv_ib = body_b.inv_inertia_world;
        sim.fixed_rotation = det(sim.inv_ia + sim.inv_ib) < 1000.0 * f32::MIN_POSITIVE;
        sim.frame_a = Pose::new(
            body_a
                .pose
                .rotate(self.local_frame_a.position - body_a.local_center),
            crate::math::mul_quat(body_a.pose.rotation, self.local_frame_a.rotation),
        );
        sim.frame_b = Pose::new(
            body_b
                .pose
                .rotate(self.local_frame_b.position - body_b.local_center),
            crate::math::mul_quat(body_b.pose.rotation, self.local_frame_b.rotation),
        );
        sim.delta_center = body_b.center - body_a.center;
        let sim = self.sim;
        match &mut self.kind {
            JointKind::Spherical(joint) => joint.prepare(&sim, ctx),
            JointKind::Revolute(joint) => joint.prepare(&sim, ctx),
            JointKind::Weld(joint) => joint.prepare(&sim, ctx),
            JointKind::Distance(joint) => joint.prepare(&sim, ctx),
            JointKind::Filter => {}
        }
    }

    /// Applies the impulses of the previous step.
    pub(crate) fn warm_start(&mut self, states: &mut [BodyState]) {
        let sim = self.sim;
        match &mut self.kind {
            JointKind::Spherical(joint) => joint.warm_start(&sim, states),
            JointKind::Revolute(joint) => joint.warm_start(&sim, states),
            JointKind::Weld(joint) => joint.warm_start(&sim, states),
            JointKind::Distance(joint) => joint.warm_start(&sim, states),
            JointKind::Filter => {}
        }
    }

    /// Solves the velocity constraints of the joint, with the position bias when `use_bias`.
    pub(crate) fn solve(&mut self, states: &mut [BodyState], ctx: &StepContext, use_bias: bool) {
        let sim = self.sim;
        match &mut self.kind {
            JointKind::Spherical(joint) => joint.solve(&sim, states, ctx, use_bias),
            JointKind::Revolute(joint) => joint.solve(&sim, states, ctx, use_bias),
            JointKind::Weld(joint) => joint.solve(&sim, states, ctx, use_bias),
            JointKind::Distance(joint) => joint.solve(&sim, states, ctx, use_bias),
            JointKind::Filter => {}
        }
    }

    /// Whether one of the two bodies is dynamic, which a joint needs to do anything.
    pub(crate) fn has_dynamic_body(body_a: &Body, body_b: &Body) -> bool {
        body_a.body_type == BodyType::Dynamic || body_b.body_type == BodyType::Dynamic
    }
}
