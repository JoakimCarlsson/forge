//! Registration and scheduling of authored scene components.

use fr_app::ecs as bevy_ecs;
use fr_app::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use fr_app::{Plugin, PostUpdate, Runtime};

use crate::hierarchy::update_transforms;
use crate::{
    AssetPath, HierarchyStatus, LocalTransform, ModelRef, Name, Parent, SceneCamera, SceneId,
    SceneLight,
};

/// The ordered scene processing stages.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SceneSystems {
    /// Computes world transforms after application systems update local values.
    TransformPropagation,
}

/// Registers editable components and propagates transforms after updates.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    /// Installs reflection metadata and hierarchy propagation.
    fn build(&self, app: &mut Runtime) {
        app.register_type::<SceneId>()
            .register_type::<Name>()
            .register_type::<Parent>()
            .register_type::<LocalTransform>()
            .register_type::<AssetPath>()
            .register_type::<ModelRef>()
            .register_type::<SceneCamera>()
            .register_type::<SceneLight>()
            .init_resource::<HierarchyStatus>()
            .add_systems(
                PostUpdate,
                update_transforms.in_set(SceneSystems::TransformPropagation),
            );
    }
}
