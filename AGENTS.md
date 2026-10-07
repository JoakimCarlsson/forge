# Forge

A general-purpose, data-driven Rust game engine on winit and wgpu. Keep the
engine reusable: engine code never depends on tooling, and never contains
game-specific rules, content, controls or scenes.

Read this file before changing the repository. It holds the rules. The
explanation of how the engine works lives in [docs](docs/), and architectural
detail belongs there, not here.

## Commands

```sh
make build # cargo build --workspace --all-targets
make fmt   # cargo fmt --all
make lint  # cargo fmt --all --check and cargo clippy --workspace --all-targets -- -D warnings
make test  # cargo test --workspace
make clean # cargo clean
make run   # cargo run --release -p fr_editor; ARGS passes arguments, e.g. ARGS=~/Documents/forge/demo
make debug # The same, for a debug editor
make env   # cargo install the editor (forge_editor)
make -C <project> run # Run a game project in release; ARGS passes arguments, e.g. ARGS="--scene scenes/demo.scene"
```

Cargo commands run at the repository root, which is the Cargo workspace.
Requirements are in [docs/getting-started.md](docs/getting-started.md).

## Repository structure

```text
Cargo.toml   Cargo workspace root: members are crates/*; Cargo.lock, rustfmt.toml and clippy.toml sit beside it
crates/      The workspace members, all prefixed fr_ (see the layers below)
docs/        Guides
Makefile     Commands above
```

The crates, from the bottom up; a crate depends only on crates of lower layers
(and on those listed):

1. `fr_math`: the one math crate. glam's `Vec2`/`Vec3`/`Vec4`/`Quat`/`Mat3`/`Mat4` re-exported, `Dir3`, `Plane`, `Aabb`, `Rect`/`Point`/`Size`, `look_at`/`perspective`/`orthographic`, and `Ray3d` with `point_at` and ray-vs-plane/sphere/AABB/triangle tests. Only dependency: glam.
2. `fr_handle`, `fr_time`, `fr_color`, `fr_input`: leaves with no engine dependencies. Typed index handles; `FrameClock` and `FixedStepper`; `Rgba` and sRGB decoding; pointer, scroll and keyboard value types and the `InputState` a frame's events are gathered into.
3. `fr_transform` (`Transform`, `Transform::looking_at`; math), `fr_image` (`ImageData`, `SamplerData`, `TextureId`; handle).
4. `fr_mesh` (`MeshData`, `MeshId`, `cube`/`sphere`/`plane`/`cylinder`/`capsule`; math, handle), `fr_material` (`MaterialData`, `MaterialId`; math, color, handle, image), `fr_light` (`DirectionalLight`, `PointLight`, `SpotLight`, `Light`; math, transform), `fr_skeleton` (`Skeleton`, `Skeleton::humanoid`; math, transform), `fr_camera` (`Camera`, `Projection`, `Frustum`, `Viewport`, viewport-to-world ray, world-to-viewport, `OrbitController`; math).
5. `fr_document`: the authored data. JSON over serde_json, `Guid`, `PropertyValue`, `EntityRecord`/`ComponentRecord`/`PrefabInstanceRecord`/`EntityDocument`, `ComponentSchema` and the `SchemaSet` with the built-in schemas (`builtin_schema.rs`), `validate_document`, the `.scene` and `.prefab` formats, `.meta` sidecars, the `AssetIndex`, atomic file replacement and the game schema export. Depends on `fr_math` and `fr_transform`.
6. `fr_assets`: the glTF/GLB importer (`load_gltf`, `KHR_lights_punctual`, `load_skeleton`, `ModelData`) and the typed `AssetLibrary` over a project's asset root: models, prefabs and scenes by guid or path, cached, with `invalidate` as the hot-reload hook. Depends on `fr_document` and the data crates, never on a device.
7. `fr_project`: `project.forge`, the project listing, scaffolding of a new game from `crates/fr_project/templates` and cargo build steps. Depends on `fr_document`.
8. `fr_physics`: rigid body physics, a scalar single threaded port of Box3D's soft step solver. Depends on `fr_math`, `fr_transform` and `fr_skeleton` only. Modules: `geometry`/`hull`, `distance` (GJK), `sat`/`manifold`, `tree`/`broad_phase`, `body`/`shape`/`contact`, `contact_solver`/`solver`/`step`/`finalize`, `joint`, `island`, `continuous`, `sensor`, `query` (ray casts take `fr_math::Ray3d`) and `ragdoll`. `World` is the entry point; handles are generational (`slot`). Its `math` module holds only solver helpers (`Pose`, `Softness`, quaternion and matrix routines with fixed operand order).
9. `fr_window`: the winit window and event loop; converts winit events into `fr_input` types. Depends on `fr_input`.
10. `fr_render`: the wgpu device, swapchain, glyph atlas, text shaping, quad and glyph pipelines, the `DrawList` a frame is submitted as (quads, text, icons, rotated quads and `line`), and the forward 3D pass (uploads, `Scene`, PBR, lights, shadows, an optional sub-viewport via `render_in`); `Renderer::new_offscreen` and `capture` render headless. Depends on `fr_math`, `fr_color`, `fr_transform`, `fr_light`, `fr_mesh`, `fr_image`, `fr_material` and `fr_camera`; never on `fr_window` or `fr_ui`.
11. `fr_ui`: the element tree, layout, hit testing, focus and input routing (click, double click, secondary press, captured drags, wheel, layers for popups) over `fr_render`'s draw list and `fr_input`; style, the one fixed `Theme`, SVG icons (artwork in `assets/icons` with its `LICENSES`), `Div`, `Text` and the generic widgets: scroll area, split and sash, tabs, menus, field with the pure `TextEdit`, drag value, combo, tree rows, colour field, slider and the dock (`DockTree` and `dock_view`), none of which knows what it shows.
12. `fr_scene`: the scene. The entity `Hierarchy` over component records, the `Stage` (the live scene: component stores, the adapters of the built-in component types, the physics world, `step_physics`, `resolve_transforms`, `apply_transform_interpolation`, `queue_destroy_entity`/`flush_destroy_queue`), realization of scenes and prefabs, and `build_render_scene`, which describes a frame as `fr_render`'s `Scene` through the `RenderResources` trait, plus `GpuResources` and `ResourceCache` (module `gpu`), the `RenderResources` that uploads to and caches on a `fr_render::Renderer`, shared by the host and the editor. Depends on `fr_document`, `fr_assets`, `fr_physics`, `fr_render` and the data crates; never on `fr_window` or `fr_ui`.
13. `fr_engine`: the facade a game depends on (`App`, `Behavior`, `ComponentRegistry`, `Runtime`, `Simulation`, `run`, `RunOptions`, `parse_run_options`). Wires every crate and re-exports each under a short name: `math`, `color`, `time`, `transform`, `input`, `light`, `mesh`, `image`, `material`, `camera`, `skeleton`, `assets`, `document`, `project`, `physics`, `scene`, `render`, `ui`. `Scene` is `fr_scene::Stage`. The host calls `App::fixed_update` at `App::fixed_timestep` (60 Hz by default) from a `FixedStepper`; the order of a frame is in [docs/architecture.md](docs/architecture.md). The UI view hook is `App::view`, and the game's interface is `fr_ui`.

Above the engine crates sit the tools; no engine crate depends on them and a
game never does either:

14. `fr_authoring`: the headless authoring layer, depending only on `fr_document`, `fr_project`, `fr_math` and `fr_transform`. Open documents (scene, prefab), commands (create, delete, duplicate, reparent, rename, set transform, add and remove component, set property), each one undo entry, history with gestures and coalescing, selection, schema-driven editing rules, saving with dirty tracking, the `Workspace` of open documents, project file operations and the `launch_plan` of Play.
15. `fr_editor`: the editor, a library and the `forge_editor` binary: panels (hierarchy, project, inspector, console, viewport), the dock, the live preview over `fr_scene` that never simulates, the transform gizmo, the per-subsystem contributions in `subsystems/` and the Play runner that launches the game as a separate process. Depends on `fr_authoring`, `fr_ui`, `fr_scene`, `fr_render`, `fr_window` and the engine crates; the editor never links a game.

No game lives in this repository. A game is a **project**: a directory of glTF
models, prefabs and scenes with a `project.forge`, plus its own Cargo project
that depends on the `fr_engine` crate, kept outside the repository (the demo is
`~/Documents/forge/demo`). The game's world is scene data; its Rust is
behaviours and an application.

## Rules

- Dependencies point downwards only, in the layer order above; no cycles. The
  direction is always `game -> fr_engine -> engine crates` and, from stage B,
  `editor -> fr_authoring -> engine crates`, and never the reverse. `fr_ui`
  and `fr_render` never know winit or `fr_window`; `fr_physics` knows nothing
  renderer-related; adding a dependency upward, or between unrelated subsystems,
  is an architectural change. Internal crates are declared once in
  `[workspace.dependencies]` and used as `fr_x.workspace = true`; add external
  dependencies with `cargo add`.
- Shared maths lives in `fr_math` only. A ray is `fr_math::Ray3d` with its
  primitive intersection tests there; casting against a physics world lives in
  `fr_physics::query`, and building a ray from a camera and pointer lives in
  `fr_camera`.
- `fr_document`, `fr_assets` and `fr_project` are the only crates that open
  files, and `fr_assets::AssetLibrary` is the engine's only way to read a
  project's files at runtime. No filesystem root reaches the `Stage`.
- winit and wgpu types never appear in the public API of `fr_engine`, `fr_ui`,
  `fr_input` or `fr_scene`. wgpu stays inside `fr_render`, winit inside
  `fr_window`. `Renderer` and the window are internal to the host `fr_engine`
  implements and must not reach a game.
- The data crates, `fr_document`, `fr_assets`, `fr_project`, `fr_physics`,
  `fr_scene` and `Simulation` stay usable without a window or graphics device,
  which is what keeps their tests headless. The stage reaches the device only
  through `RenderResources`.
- `fr_physics` is deterministic: iterate in handle or index order, never in hash
  map order (hash sets are only for membership), keep the operand order of every
  float expression and never fuse a multiply with an add (no `mul_add`). Its code
  follows Box3D's structure; keep the C names in mind when porting more. The
  stage iterates its stores in handle order, for the same reason.
- Initialization failures return an error, log `forge: <message>` and the binary
  exits nonzero. Setup, the project, the startup scene and `App::start`, runs
  before a window exists.
- `Stage::build_render_scene` borrows the stage and runs after everything that
  can mutate it, including the user interface. Entity removal during play goes
  through `Stage::queue_destroy_entity`, and `Runtime::structural_boundary` is the
  single boundary after the user interface and before the frame is described where
  destruction, spawning and scene replacement all happen; immediate
  `Stage::destroy_entity` is only for setup and teardown. The order is in
  [docs/architecture.md](docs/architecture.md).
- `Stage::resolve_transforms` and `Stage::apply_transform_interpolation` are the
  last two calls before `build_render_scene`, in that order. Setting an entity
  transform only marks its subtree dirty, so nothing may read a
  transform-consuming component without a resolve first, and interpolation may
  write render transforms only: never an entity transform, a cached global or a
  physics body.
- Authored data is entities with component records, identified by persistent
  `Guid`s, in versioned JSON `.scene` and `.prefab` documents with committed
  `.meta` sidecars. Component types are registered with their schema, defaults,
  multiplicity and dependency rules, and realized through registered adapters;
  shared document, hierarchy and realization code carries no per-component-type
  switches. A new built-in component type is one schema in `builtin_schema.rs`
  and one adapter in `fr_scene`. The contracts are in
  [docs/authoring.md](docs/authoring.md).
- Versions detect mismatch; they never imply compatibility. Do not add
  old-version readers, field aliases, coercions or migration chains: reject an
  incompatible file with a diagnostic naming the version found and required.
  Data whose schema is unavailable is preserved across a save, never deleted.
- The editor's interface is `fr_ui`. Reusable controls (layout, text fields, drag values, combos, trees, tabs, menus, scroll areas, splits, the dock, popups) belong in `fr_ui`, which knows no document or schema; widgets that know the editor's data (property rows over component schemas, asset slots, the panels) stay in `fr_editor`, built from `fr_ui` primitives. The editor runs its own loop on `fr_window` and draws through `fr_render::Renderer`; the engine hosts no editor interface.
- Editor layers depend downwards only: panels -> viewport -> preview -> `fr_authoring` commands -> document. Panels read state and send `fr_authoring` commands and never mutate a document directly; a drag is one gesture and one undo entry. The preview never steps physics or plays a behaviour: nothing simulates while authoring.
- Adding support for a new engine subsystem in the editor means adding one folder under `crates/fr_editor/src/subsystems/` and registering it in `register_builtin_subsystems`; the built-in components are contributed that way, and `viewport/`, `preview/` and `fr_authoring` carry no per-component switches.
- Nothing the editor owns for its own sake is written into a project's source tree: settings and the dock layout live in the per-user config directory, per-project editor state in `<project>/.forge-editor/`, which is git-ignored. Play launches the game as a separate process; no game code is loaded into the editor.
- A game registers its component types in explicit code, through
  `ComponentRegistry`, so no behaviour registers itself as a side effect of being
  linked. Nothing that belongs in a scene is hard-coded in a game's Rust.
- Crates from crates.io are allowed where a crate needs them (wgpu, winit and
  the crates around them, glam, gltf, serde_json), pinned by `Cargo.lock`.
  Shaders are WGSL inside `fr_render`. `cargo fmt --all --check` and `cargo
  clippy --workspace --all-targets -- -D warnings` stay clean, and every item is
  documented, private ones included.
- Every module file starts with a `//!` doc.
- Do not add speculative subsystems before their contracts are defined. Keep
  changes scoped.

## Rendering principles

- Shaders are `.wgsl` parts in `fr_render/src/shaders`, joined with `concat!` per pipeline (common, output, pbr, shadows, lights, mesh; shadow_depth for shadow maps). One pipeline per blend/culling variant, never per object; object transforms live in one storage buffer indexed by instance.
- Colour is linear until the output: textures marked sRGB are decoded by the GPU, data textures are not, and the mesh shader tonemaps (ACES) and encodes the swapchain format itself. Colours in component properties are linear; `MaterialData::solid` takes sRGB; the scene's `clear_color` is sRGB like the UI.
- Avoid local `var`s of struct or array type in WGSL (use constructors and arithmetic): the Vulkan validation layer rejects them under `WGPU_VALIDATION=1`, which `Renderer::new` honours.
- Light units: directional intensity is illuminance on a facing surface; point and spot intensity is illuminance at one unit, with inverse-square falloff and a smooth range window.

## UI principles

- A frame is a draw list the UI builds and submits. A new primitive is an instance struct and a shader in `fr_render`'s `pipeline`, never a render pass in a caller.
- The element tree is rebuilt every frame from the app's own state (`App::view`); nothing of it survives. What survives is `Ui`: theme, pointer, focus and the regions of the last frame.
- Elements never mutate app state. They register regions, and a click or a keypress comes back as one of the app's messages (`App::Message`), generic over the message type, delivered to `App::message` with the `AppContext`.
- Styling is Tailwind-like: a spacing scale of 4 logical pixels per step, one chainable `Styled` setter per utility, and colours, radii, sizes and the type scale read from `Theme` tokens, never written at the call site.
- A popup or menu paints on its own layer (`push_layer`), behind it a backdrop whose press sends the caller's dismiss message; a press on a region that takes drags captures the pointer until the release, and the drag arrives as `DragEvent`s in the caller's messages. Where things were painted comes back through `Rects`.
- Widgets are extracted from screens once they repeat. A widget that paints itself is an `Element`; one that arranges others is a function returning a `Div`.
- Names are reserved for the thing they mean; crate roots are facades holding only module declarations and re-exports.

## Conventions

- Document every item, private ones included, with a rustdoc comment in the
  style of the standard library: a summary line, then `# Errors`, `# Panics` and
  `# Safety` sections where they carry information. Function bodies contain no
  comments.
- Format with `cargo fmt --all` and the repository `rustfmt.toml`. Fix clippy
  diagnostics rather than allowing lints locally; `make lint` stays clean.
- Files, modules and functions use `snake_case`, types use `PascalCase`.
- Keep public APIs small and intentional. Prefer value types and stable handles
  over exposing ownership or backend details.
- Return `Result` for failure; do not panic on bad input or a failing device.
- Extract a repeating pattern into a shared function. End every file with an
  empty line.
- Write build output to the git-ignored `build/`, never `/tmp`.

## Documentation

Changing the public API of `fr_engine`, `fr_scene` or `fr_document` means
updating the guide that describes it in the same change.

| Document | Contents |
| --- | --- |
| [docs/getting-started.md](docs/getting-started.md) | Requirements, commands, a project, a minimal game |
| [docs/architecture.md](docs/architecture.md) | Layers, dependency direction, the frame order, the boundaries, lifetime and error rules |
| [docs/editor.md](docs/editor.md) | What the editor is, its layers, panels, dock, toolkit boundary, projects, adding a subsystem and Play |
| [docs/authoring.md](docs/authoring.md) | Entities, components, scenes, prefabs, identity, versions, saving, the behaviour lifecycle |

## Verification

Run `make fmt`, `make lint`, `make build` and `make test` before finishing. Add
focused tests alongside new testable systems when asked, keep them runnable
without a window or graphics device, and still do a validation-enabled run for
rendering changes:

```sh
cd ~/Documents/forge/demo
cargo build --release
WGPU_VALIDATION=1 timeout 8 target/release/forge_demo   # no panic, no validation errors
WGPU_VALIDATION=1 target/release/forge_demo --frames 240 # ends on its own with exit code 0
```

The demo (`make -C ~/Documents/forge/demo run`) opens with the lit scene of
`assets/scenes/demo.scene`: a ground, lit primitives, a pile of rigid bodies,
two ragdolls and the panel in the top-left corner. Dragging with the primary
button orbits the camera and scrolling zooms. The panel's switches turn the
display objects, the shadows and V-sync on and off; Reset reloads the scene,
Pause stops the physics, and Ragdoll and Box spawn prefabs above the pile.

Check the data and simulation headlessly with a throwaway program over
`fr_engine::Simulation` (kept in `~/.cache/scratch/forge`, never committed):
open the project, load the startup scene, step `fixed_update` until the bodies
sleep, read the stage, write the scene document and read it back, and check that
a changed `format_version` is rejected with a diagnostic naming both versions.

The editor is verified headlessly with a capture, which renders it offscreen so
no display or pointer is needed; read the PNG, then run it windowed under
validation:

```sh
cargo run --release -p fr_editor -- ~/Documents/forge/demo --capture build/editor.png --frames 6 --size 1600x900
WGPU_VALIDATION=1 timeout 8 target/release/forge_editor ~/Documents/forge/demo
```

Check `fr_authoring` with a throwaway program in `~/.cache/scratch/forge` on a
copy of the demo's assets: create, set a property, undo and redo, save, reload
and compare, and print `launch_plan`. Never edit the real demo in a check.
