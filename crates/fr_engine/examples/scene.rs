//! A headless ECS application that moves a hierarchy and round-trips reflected scene data.

use std::error::Error;

use fr_engine::app::{FixedUpdate, Startup};
use fr_engine::ecs::{
    self as bevy_ecs,
    component::Component,
    reflect::ReflectComponent,
    system::{Commands, Query, Res},
};
use fr_engine::math::Vec3;
use fr_engine::reflect::{self as bevy_reflect, Reflect};
use fr_engine::scene::{GlobalTransform, LocalTransform, Name, Parent, SceneDocument, SceneId};
use fr_engine::transform::Transform;
use fr_engine::{App, FixedStep, Frame, Plugin, Runtime};

/// A movement velocity that is editable and saved with its object.
#[derive(Component, Reflect, Clone)]
#[reflect(Component)]
struct Velocity {
    /// Metres travelled per second along each axis.
    metres_per_second: [f32; 3],
}

/// The systems and reflected types used by this example.
struct MovementPlugin;

impl Plugin for MovementPlugin {
    /// Registers movement data and adds the fixed simulation system.
    fn build(&self, app: &mut Runtime) {
        app.register_type::<Velocity>()
            .add_systems(FixedUpdate, move_objects);
    }
}

/// Spawns a moving parent and a child with a stable parent reference.
fn spawn_objects(mut commands: Commands) {
    let root = SceneId::new();
    commands.spawn((
        root.clone(),
        Name(String::from("Parent")),
        LocalTransform::default(),
        Velocity {
            metres_per_second: [1.0, 0.0, 0.0],
        },
    ));
    commands.spawn((
        SceneId::new(),
        Name(String::from("Child")),
        Parent(root),
        LocalTransform::new(Transform::from_translation(Vec3::Y)),
    ));
}

/// Advances local positions in the fixed simulation stage.
fn move_objects(step: Res<FixedStep>, mut objects: Query<(&Velocity, &mut LocalTransform)>) {
    for (velocity, mut transform) in &mut objects {
        let position = Vec3::from_array(transform.translation)
            + Vec3::from_array(velocity.metres_per_second) * step.delta_seconds;
        transform.translation = position.to_array();
    }
}

/// Returns a window-independent frame description.
fn frame(delta_seconds: f32) -> Frame {
    Frame {
        delta_seconds,
        index: 1,
        width: 1,
        height: 1,
        scale_factor: 1.0,
        interpolation: 0.0,
    }
}

/// Runs fixed updates, encodes scene data and restores it in a fresh ECS world.
fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::<()>::new();
    app.add_plugins(MovementPlugin)
        .add_systems(Startup, spawn_objects);
    app.update(frame(1.0 / 30.0))?;
    let source = app.capture_scene()?.to_json()?;
    let document = SceneDocument::from_json(&source)?;
    let mut restored = App::<()>::new();
    restored.add_plugins(MovementPlugin);
    restored.spawn_scene(&document)?;
    restored.update(frame(0.0))?;
    let mut query = restored.world_mut().query::<(&Name, &GlobalTransform)>();
    let mut objects: Vec<_> = query
        .iter(restored.world())
        .map(|(name, transform)| (name.0.clone(), transform.0.transform_point3(Vec3::ZERO)))
        .collect();
    objects.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, position) in objects {
        println!("{name}: {position}");
    }
    Ok(())
}
