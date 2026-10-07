//! The ragdoll adapter.

use fr_document::ComponentRecord;
use fr_document::builtin_schema::RAGDOLL;
use fr_physics::ragdoll::{Ragdoll, RagdollDef};
use fr_skeleton::Skeleton;

use super::{ComponentAdapter, RealizeContext, props_of};
use crate::error::SceneError;
use crate::hierarchy::EntityHandle;
use crate::resources::MaterialSpec;
use crate::stage::Stage;

/// The live state of a ragdoll component.
pub struct RagdollState {
    /// The bodies and joints in the stage's physics world.
    pub ragdoll: Ragdoll,
    /// The material the limbs are drawn with.
    pub limbs: MaterialSpec,
    /// The material of the bone markers, when they are shown.
    pub bones: Option<MaterialSpec>,
}

/// Realizes `forge.ragdoll`: the bodies and joints of a humanoid built by
/// `fr_physics` at the entity's placement. Each ragdoll gets its own collision
/// group, so its limbs never collide with each other.
pub struct RagdollAdapter;

impl ComponentAdapter for RagdollAdapter {
    /// The ragdoll type key.
    fn key(&self) -> &'static str {
        RAGDOLL
    }

    /// Builds the ragdoll at the entity's resolved transform.
    fn realize(
        &self,
        context: &mut RealizeContext<'_>,
        entity: EntityHandle,
        record: &ComponentRecord,
    ) -> Result<(), SceneError> {
        let props = props_of(context.stage, record)?;
        let color = props.color("color");
        let bone_color = props.color("bone_color");
        let limbs = MaterialSpec {
            roughness: props.float("roughness"),
            ..MaterialSpec::solid([color.r, color.g, color.b], 0.0, 0.6)
        };
        let bones = props
            .boolean("show_bones")
            .then(|| MaterialSpec::solid([bone_color.r, bone_color.g, bone_color.b], 0.0, 0.4));
        let friction_torque = props.float("friction_torque");
        let joint_hertz = props.float("joint_hertz");
        let joint_damping_ratio = props.float("joint_damping_ratio");
        let density = props.float("density");
        let stage = &mut *context.stage;
        let global = stage
            .global_transform(entity)
            .ok_or(SceneError::MissingEntity)?;
        stage.next_ragdoll_group += 1;
        let definition = RagdollDef {
            position: global.translation,
            rotation: global.rotation,
            group: stage.next_ragdoll_group,
            friction_torque,
            joint_hertz,
            joint_damping_ratio,
            density,
        };
        let ragdoll = Ragdoll::build(&mut stage.world, &Skeleton::humanoid(), &definition)
            .map_err(|error| SceneError::realization(format!("the ragdoll: {error}")))?;
        stage.ragdolls.insert(
            entity,
            RagdollState {
                ragdoll,
                limbs,
                bones,
            },
        );
        Ok(())
    }

    /// Destroys the ragdoll's bodies and joints.
    fn release(&self, stage: &mut Stage, entity: EntityHandle) {
        if let Some(state) = stage.ragdolls.remove(&entity) {
            state.ragdoll.destroy(&mut stage.world);
        }
    }
}
