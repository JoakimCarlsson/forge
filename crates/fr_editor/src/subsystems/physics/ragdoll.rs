//! The contribution for `forge.ragdoll`.

use fr_authoring::ComponentInit;
use fr_document::builtin_schema::RAGDOLL;
use fr_ui::IconName;

use crate::subsystems::{CreateEntry, Subsystem};

/// The group of the physics entries in the Create menu.
const PHYSICS_CATEGORY: &str = "Physics";

/// Contributes the ragdoll, a humanoid the simulation builds, so it has no
/// bounds or shape here.
pub struct RagdollSubsystem;

impl Subsystem for RagdollSubsystem {
    /// The ragdoll's type key.
    fn kind(&self) -> &'static str {
        RAGDOLL
    }

    /// The hierarchy icon, a body of joined parts.
    fn icon(&self) -> IconName {
        IconName::Hierarchy
    }

    /// The Ragdoll entry.
    fn create_entries(&self) -> Vec<CreateEntry> {
        vec![
            CreateEntry::new("Ragdoll", PHYSICS_CATEGORY, IconName::Hierarchy, "Ragdoll")
                .with_component(ComponentInit::new(RAGDOLL)),
        ]
    }
}
