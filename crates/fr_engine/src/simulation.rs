//! Optional ECS integration of the deterministic physics world.

use fr_app::ecs as bevy_ecs;
use fr_app::ecs::{
    component::Component,
    resource::Resource,
    schedule::{IntoScheduleConfigs, SystemSet},
    system::{Query, Res, ResMut},
};
use fr_app::{FixedUpdate, Plugin, PostUpdate, Runtime};
use fr_physics::body::BodyId;
use fr_physics::world::{World, WorldDef};
use fr_scene::{LocalTransform, SceneSystems};

use crate::{FixedStep, Frame};

/// A deterministic physics world shared by simulation systems.
#[derive(Resource)]
pub struct PhysicsWorld(pub World);

impl Default for PhysicsWorld {
    /// Creates a physics world with default gravity and solver settings.
    fn default() -> Self {
        Self(World::new(WorldDef::default()))
    }
}

/// Fixed-step solver settings, editable while the app runs.
#[derive(Resource, Clone, Debug)]
pub struct PhysicsSettings {
    /// The number of solver substeps in each fixed update.
    pub sub_steps: usize,
    /// Whether the physics solver is paused.
    pub paused: bool,
}

impl Default for PhysicsSettings {
    /// Uses four substeps with simulation enabled.
    fn default() -> Self {
        Self {
            sub_steps: 4,
            paused: false,
        }
    }
}

/// A runtime body that supplies an entity's root transform.
///
/// Physics poses are world-space; entities using this component must be roots.
#[derive(Component, Clone, Copy, Debug)]
pub struct RigidBody(pub BodyId);

/// Explicit ordering points around the deterministic solver.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PhysicsSystems {
    /// Changes forces, targets or bodies before the solver runs.
    BeforeStep,
    /// Advances the physics world once.
    Step,
    /// Reads the simulation results after the solver runs.
    AfterStep,
    /// Writes interpolated body poses before transform propagation.
    Writeback,
}

/// Installs the fixed-step solver and interpolated root transform writeback.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    /// Registers world resources and explicit simulation ordering.
    fn build(&self, app: &mut Runtime) {
        app.init_resource::<PhysicsWorld>()
            .init_resource::<PhysicsSettings>()
            .configure_sets(
                FixedUpdate,
                (
                    PhysicsSystems::BeforeStep,
                    PhysicsSystems::Step,
                    PhysicsSystems::AfterStep,
                )
                    .chain(),
            )
            .configure_sets(
                PostUpdate,
                PhysicsSystems::Writeback.before(SceneSystems::TransformPropagation),
            )
            .add_systems(FixedUpdate, step_physics.in_set(PhysicsSystems::Step))
            .add_systems(
                PostUpdate,
                write_body_poses.in_set(PhysicsSystems::Writeback),
            );
    }
}

/// Steps the scalar solver with the host's fixed duration.
fn step_physics(
    mut world: ResMut<PhysicsWorld>,
    settings: Res<PhysicsSettings>,
    step: Res<FixedStep>,
) {
    if !settings.paused {
        world.0.step(step.delta_seconds, settings.sub_steps.max(1));
    }
}

/// Writes interpolated physics poses while retaining each object's visual scale.
fn write_body_poses(
    world: Res<PhysicsWorld>,
    settings: Res<PhysicsSettings>,
    frame: Res<Frame>,
    mut bodies: Query<(&RigidBody, &mut LocalTransform)>,
) {
    let alpha = if settings.paused {
        1.0
    } else {
        frame.interpolation
    };
    for (body, mut local) in &mut bodies {
        if let Some(pose) = world.0.body_interpolated_pose(body.0, alpha) {
            local.translation = pose.position.to_array();
            local.rotation = pose.rotation.to_array();
        }
    }
}
