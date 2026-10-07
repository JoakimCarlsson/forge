//! Stepping the physics world and moving entities with it.

use std::collections::BTreeMap;

use fr_physics::body::BodyType;
use fr_physics::math::Pose;
use fr_transform::Transform;

use crate::hierarchy::EntityHandle;
use crate::stage::Stage;

impl Stage {
    /// Advances the physics world by one fixed step of `seconds`.
    ///
    /// Transforms resolve first, and kinematic and static bodies take the pose
    /// of their entity. After the step every awake simulated body writes its
    /// pose back to its entity, as a local pose under the entity's parent, so
    /// parented bodies keep a meaningful local transform; descendants of a
    /// moved entity are marked stale.
    pub fn step_physics(&mut self, seconds: f32) {
        self.hierarchy.resolve_transforms();
        self.push_entity_poses();
        self.world
            .step(seconds, self.settings.physics.sub_steps as usize);
        self.write_back_poses();
    }

    /// Gives every kinematic and static body the pose of its entity when it
    /// differs.
    fn push_entity_poses(&mut self) {
        let moved: Vec<_> = self
            .bodies
            .iter()
            .filter(|(_, state)| state.body_type != BodyType::Dynamic)
            .filter_map(|(entity, state)| {
                let global = self.hierarchy.get(*entity)?.global;
                let current = self.world.body_pose(state.body)?;
                let wanted = Pose::new(global.translation, global.rotation);
                (current.position != wanted.position || current.rotation != wanted.rotation)
                    .then_some((state.body, wanted))
            })
            .collect();
        for (body, pose) in moved {
            self.world.set_body_pose(body, pose);
        }
    }

    /// Writes the pose of every awake dynamic body to its entity.
    fn write_back_poses(&mut self) {
        let poses: Vec<_> = self
            .bodies
            .iter()
            .filter(|(_, state)| state.body_type == BodyType::Dynamic)
            .filter(|(_, state)| self.world.is_body_awake(state.body))
            .filter_map(|(entity, state)| Some((*entity, self.world.body_pose(state.body)?)))
            .collect();
        for (entity, pose) in poses {
            let Some(scale) = self.hierarchy.get(entity).map(|found| found.global.scale) else {
                continue;
            };
            self.hierarchy.set_global(
                entity,
                Transform {
                    translation: pose.position,
                    rotation: pose.rotation,
                    scale,
                },
            );
        }
    }

    /// Sets the transforms the frame draws: bodies at the pose between their
    /// last two steps, `alpha` of the way from the earlier to the later, and
    /// everything under them following. It writes nothing the simulation reads:
    /// no entity transform, no cached global and no body.
    ///
    /// The frame loop calls it right after [`Stage::resolve_transforms`].
    pub fn apply_transform_interpolation(&mut self, alpha: f32) {
        self.hierarchy.resolve_transforms();
        self.interpolation = alpha;
        let world = &self.world;
        let driven: BTreeMap<EntityHandle, Transform> = self
            .bodies
            .iter()
            .filter(|(_, state)| state.body_type == BodyType::Dynamic)
            .filter_map(|(entity, state)| {
                let pose = world.body_interpolated_pose(state.body, alpha)?;
                let scale = self.hierarchy.get(*entity)?.global.scale;
                Some((
                    *entity,
                    Transform {
                        translation: pose.position,
                        rotation: pose.rotation,
                        scale,
                    },
                ))
            })
            .collect();
        self.hierarchy
            .compute_render_transforms(&|entity| driven.get(&entity).copied());
    }
}
