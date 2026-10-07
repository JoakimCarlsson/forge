# Forge

A general-purpose Rust game engine on winit and wgpu. Keep the engine reusable:
engine code never contains game-specific rules, content or scenes.

## Commands

```sh
make run   # Run the example (EXAMPLE=demo, the only example) in release; ARGS passes arguments, e.g. make run ARGS=path/to/model.glb
make debug # Run the example in debug
make build # cargo build --workspace --all-targets
make fmt   # cargo fmt --all
make lint  # cargo fmt --all --check and cargo clippy --workspace --all-targets -- -D warnings
make test  # cargo test --workspace
```

## Structure

```text
Cargo.toml   Workspace root: members are crates/*; rustfmt.toml and clippy.toml sit beside it
crates/      The workspace members, all prefixed fr_ (see the layers below)
```

Layers from the bottom up; a crate depends only on crates of lower layers (and on those listed):

1. `fr_math`: the one math crate. glam's `Vec2`/`Vec3`/`Vec4`/`Quat`/`Mat3`/`Mat4` re-exported, `Dir3`, `Plane`, `Aabb`, `Rect`/`Point`/`Size`, `look_at`/`perspective`/`orthographic`, and `Ray3d` with `point_at` and ray-vs-plane/sphere/AABB/triangle tests. Only dependency: glam.
2. `fr_handle`, `fr_time`, `fr_color`, `fr_input`: leaves with no engine dependencies. Typed index handles; `FrameClock` and `FixedStepper`; `Rgba` and sRGB decoding; pointer, scroll and keyboard value types.
3. `fr_transform` (`Transform`; math), `fr_image` (`ImageData`, `SamplerData`, `TextureId`; handle).
4. `fr_mesh` (`MeshData`, `MeshId`, `cube`/`sphere`/`plane`/`cylinder`/`capsule`; math, handle), `fr_material` (`MaterialData`, `MaterialId`; math, color, handle, image), `fr_light` (`DirectionalLight`, `PointLight`, `SpotLight`, `Light`; math, transform), `fr_skeleton` (`Skeleton`, `Skeleton::humanoid`; math, transform), `fr_camera` (`Camera`, `Projection`, `Frustum`, `Viewport`, viewport-to-world ray, world-to-viewport, `OrbitController`; math).
5. `fr_assets`: loaders only: the glTF/GLB importer (`load_gltf`, `KHR_lights_punctual`, `load_skeleton`) and `ModelData`. Depends on the data crates, never on a device.
6. `fr_physics`: rigid body physics, a scalar single threaded port of Box3D's soft step solver. Depends on `fr_math`, `fr_transform` and `fr_skeleton` only. Modules: `geometry`/`hull`, `distance` (GJK), `sat`/`manifold`, `tree`/`broad_phase`, `body`/`shape`/`contact`, `contact_solver`/`solver`/`step`/`finalize`, `joint`, `island`, `continuous`, `sensor`, `query` (ray casts take `fr_math::Ray3d`) and `ragdoll`. `World` is the entry point; handles are generational (`slot`). Its `math` module holds only solver helpers (`Pose`, `Softness`, quaternion and matrix routines with fixed operand order).
7. `fr_window`: the winit window and event loop; converts winit events into `fr_input` types. Depends on `fr_input`.
8. `fr_render`: the wgpu device, swapchain, glyph atlas, text shaping, quad and glyph pipelines, the `DrawList` a frame is submitted as, and the forward 3D pass (uploads, `Scene`, PBR, lights, shadows). Depends on `fr_math`, `fr_color`, `fr_transform`, `fr_light`, `fr_mesh`, `fr_image`, `fr_material` and `fr_camera`; never on `fr_window` or `fr_ui`.
9. `fr_ui`: the element tree, layout, hit testing, focus and input routing over `fr_render`'s draw list and `fr_input`; style, the one fixed `Theme`, SVG icons (artwork in `assets/icons` with its `LICENSES`), `Div`, `Text` and the widgets.
10. `fr_engine`: the facade a game depends on (`App`, `run`, `Assets` with `load_gltf`). Wires every crate and re-exports each under a short name: `math`, `color`, `time`, `transform`, `input`, `light`, `mesh`, `image`, `material`, `camera`, `skeleton`, `assets`, `physics`, `render`, `ui`. The host calls `App::fixed_update` at `App::fixed_timestep` (60 Hz by default) from a `FixedStepper`; `Frame::interpolation` blends between steps; a game owns its `physics::world::World`. `App::scene` describes the 3D frame, `App::vsync` chooses vsync. The `demo` example (`crates/fr_engine/examples`) is the lit PBR scene, glTF via ARGS, UI overlay and the physics pile with ragdolls; the standalone game in `~/Documents/forge/demo` mirrors it.

## Rules

- Dependencies point downwards only, in the layer order above; no cycles. `fr_ui` and `fr_render` never know winit or `fr_window`; `fr_physics` knows nothing renderer-related; `fr_assets` is the only crate that reads files. Internal crates are declared once in `[workspace.dependencies]` and used as `fr_x.workspace = true`.
- Shared maths lives in `fr_math` only. A ray is `fr_math::Ray3d` with its primitive intersection tests there; casting against a physics world lives in `fr_physics::query`, and building a ray from a camera and pointer lives in `fr_camera`.
- winit and wgpu types never appear in the public API of `fr_engine`, `fr_ui`, or `fr_input`. wgpu stays inside `fr_render`, winit inside `fr_window`.
- The data crates (layers 1 to 4) stay usable without a window or graphics device, which keeps their tests headless.
- `fr_physics` is deterministic: iterate in handle or index order, never in hash map order (hash sets are only for membership), keep the operand order of every float expression and never fuse a multiply with an add (no `mul_add`). Its code follows Box3D's structure; keep the C names in mind when porting more.
- Initialization failures return an error and the binary exits nonzero.
- Every module file starts with a `//!` doc.
- Do not add speculative subsystems before their contracts are defined. Keep changes scoped.

## Rendering principles

- Shaders are `.wgsl` parts in `fr_render/src/shaders`, joined with `concat!` per pipeline (common, output, pbr, shadows, lights, mesh; shadow_depth for shadow maps). One pipeline per blend/culling variant, never per object; object transforms live in one storage buffer indexed by instance.
- Colour is linear until the output: textures marked sRGB are decoded by the GPU, data textures are not, and the mesh shader tonemaps (ACES) and encodes the swapchain format itself.
- Avoid local `var`s of struct or array type in WGSL (use constructors and arithmetic): the Vulkan validation layer rejects them under `WGPU_VALIDATION=1`, which `Renderer::new` honours.
- Light units: directional intensity is illuminance on a facing surface; point and spot intensity is illuminance at one unit, with inverse-square falloff and a smooth range window.

## UI principles

- A frame is a draw list the UI builds and submits. A new primitive is an instance struct and a shader in `fr_render`'s `pipeline`, never a render pass in a caller.
- The element tree is rebuilt every frame from the app's own state (`App::view`); nothing of it survives. What survives is `Ui`: theme, pointer, focus and the regions of the last frame.
- Elements never mutate app state. They register regions, and a click or a keypress comes back as one of the app's messages (`App::Message`), generic over the message type.
- Styling is Tailwind-like: a spacing scale of 4 logical pixels per step, one chainable `Styled` setter per utility, and colours, radii, sizes and the type scale read from `Theme` tokens, never written at the call site.
- Widgets are extracted from screens once they repeat. A widget that paints itself is an `Element`; one that arranges others is a function returning a `Div`.
- Names are reserved for the thing they mean; crate roots are facades holding only module declarations and re-exports.

## Conventions

- Document every item, private ones included, with a rustdoc comment in the style of the standard library. Function bodies contain no comments.
- Fix clippy diagnostics rather than allowing lints locally; `make lint` stays clean.
- Return `Result` for failure; do not panic on bad input or a failing device.
- Write build output to the git-ignored `build/`, never `/tmp`.
