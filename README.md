# forge

A general-purpose Rust game engine built on winit and wgpu. See [AGENTS.md](AGENTS.md) for the layout and rules.

```sh
make run
```

The demo: lit PBR scene with shadows, optional glTF (`make run ARGS=path/to/model.glb`), and one ragdoll simulated by `fr_physics`. Hold the right mouse button to look around and fly (W A S D, E or space up, Q or ctrl down, shift faster, wheel for speed); press the left button on the ragdoll to grab and drag it, with the wheel moving it towards or away from you. The panel resets the ragdoll and sets the strength, damping and torque limit of its joint motors.

`fr_physics` is a Rust port of the soft step solver of Box3D: spheres, capsules and boxes (convex hulls), speculative contacts with warm starting, relaxation and restitution, a dynamic AABB tree, islands with sleeping, continuous collision against static shapes, spherical, revolute, weld, distance and mouse joints, sensors, ray casts, joint motors (torque limited springs towards a target pose) and data driven rigs built on a transform hierarchy, with a humanoid template. It is scalar and single threaded, and deterministic bit for bit.

Games compose an `App<Message>` from ECS resources, systems and plugins. `Startup` loads CPU assets, `PreUpdate` handles `Inputs` and `UiMessages<Message>`, fixed schedules advance simulation, and `Extract` produces a `RenderScene`. The optional `PhysicsPlugin` supplies an ordered fixed-step solver and interpolated transforms. UI constructors registered with `App::set_view` read world state and emit messages.

`fr_app` exposes Bevy ECS, reflection and plugin contracts without a graphics device. `fr_scene` holds authored components, stable object IDs, parent relationships and versioned JSON documents. Register custom components with `#[derive(Component, Reflect)]`, `#[reflect(Component)]` and `App::register_type`; import `fr_engine::{ecs as bevy_ecs, reflect as bevy_reflect}` in modules using the derives. `App::capture_scene`/`spawn_scene` work in memory, and `save_scene`/`load_scene` handle files. Only registered reflected components on entities with `SceneId` are saved; runtime components and resources are rebuilt by systems. Authored component references use `SceneId`; ECS entity handles belong to the runtime world. Use `ModelRef(AssetPath(...))` for persistent model references and `LocalTransform` for placement. Asset paths are resolved relative to the process working directory. `MeshRenderer` references runtime assets and is omitted from scene documents.

`Assets` is an ECS resource available during every stage. Loading and CPU storage work headlessly; GPU uploads happen before presentation. Meshes, materials and textures can be replaced under their existing handles or removed explicitly. Model sources are cached by path and can be unloaded with their owned assets. Removed handles are never reused.

Run the headless custom-component and hierarchy round-trip example with:

```sh
cargo run -p fr_engine --example scene
```
