//! Data driven rigs: a [`RigDef`] lists bodies on bones and joints between them, and a [`Rig`]
//! is its instance in a [`World`](crate::world::World) on a transform hierarchy.
//!
//! Bones are the nodes of an `fr_transform` hierarchy. Every joint is a motor: a torque limited
//! spring towards a target pose, which is the bind pose until a pose is set, plus Box3D's joint
//! friction. Zero strength lets the rig go limp, a stiff motor holds a pose and an animation
//! drives the rig by setting a target pose every step. The [`humanoid_rig`] template makes a
//! definition for any hierarchy whose bones map onto the humanoid layout.

mod def;
mod humanoid;
mod instance;

pub use def::{BoneRef, RigBody, RigDef, RigJoint, RigJointKind, RigShapeDef};
pub use humanoid::{HumanoidBones, humanoid_rig, humanoid_t_pose};
pub use instance::{Rig, RigError, RigSettings};
