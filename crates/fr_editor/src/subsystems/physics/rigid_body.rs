//! The contribution for `forge.rigid_body`.

use fr_document::builtin_schema::RIGID_BODY;
use fr_ui::IconName;

use crate::subsystems::Subsystem;

/// Contributes the rigid body, which has no extent of its own: its shape comes
/// from its colliders.
pub struct RigidBodySubsystem;

impl Subsystem for RigidBodySubsystem {
    /// The rigid body's type key.
    fn kind(&self) -> &'static str {
        RIGID_BODY
    }

    /// The box icon.
    fn icon(&self) -> IconName {
        IconName::Box
    }
}
