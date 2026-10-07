//! Registration of the three physics contributions.

use std::rc::Rc;

use crate::subsystems::SubsystemRegistry;

use super::collider::ColliderSubsystem;
use super::ragdoll::RagdollSubsystem;
use super::rigid_body::RigidBodySubsystem;

/// Adds the rigid body, collider and ragdoll subsystems to the registry.
pub fn register(registry: &mut SubsystemRegistry) {
    registry.add(Rc::new(RigidBodySubsystem));
    registry.add(Rc::new(ColliderSubsystem));
    registry.add(Rc::new(RagdollSubsystem));
}
