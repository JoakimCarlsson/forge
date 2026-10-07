# Architecture

Forge is a data-driven engine. A game is a **project**: a directory of glTF
models, prefabs and scenes, plus its own Cargo project that depends on the
`fr_engine` crate. The world a game plays in is authored data, entities with
component records identified by persistent `Guid`s in versioned JSON `.scene`
and `.prefab` documents, and the game's own code is behaviours attached to
entities and an application that drives the frame. The engine defines no
characters, weapons or levels.

The scene is a hierarchy of stable `EntityHandle` values (a generational
`fr_physics::slot::Handle`). An entity owns its name, local transform, parent,
ordered children and the records of its components. Registered adapters turn
those records into live state in the `Stage`: cameras, lights, mesh renderers
and physics bodies. Physics writes a simulated body's pose back through its
entity, so a body parented under another entity keeps a meaningful local pose.

## Scope today

What the engine does:

- Reads a project: `project.forge`, an asset root of `.scene`, `.prefab` and
  glTF/GLB files, each with a committed `.meta` sidecar that gives it a `Guid`.
- Realizes a scene document into a `Stage` through the adapters of the built-in
  component types (`forge.camera`, `forge.light`, `forge.mesh_renderer`,
  `forge.rigid_body`, `forge.collider`, `forge.ragdoll`) and constructs the
  behaviours of the game's own component types through the `ComponentRegistry`.
- Steps `fr_physics` at a fixed 60 Hz from a frame-time accumulator and
  interpolates the bodies between their last two steps when drawing.
- Draws the stage through `fr_render`'s `Scene`, and the game's user interface
  from `fr_ui` over it, in one submission.
- Spawns prefabs and replaces scenes during play at one structural boundary.

What it does not do yet: a CLI, audio, animation,
particles, hot reload (the asset library has the hooks), a play mode inside any
tool, networked or streamed scenes.

## Repository structure

```text
Cargo.toml   Cargo workspace root: members are crates/*; Cargo.lock, rustfmt.toml and clippy.toml sit beside it
crates/      The workspace members, below
docs/        Guides
Makefile     run and debug (the editor), build, env, fmt, lint, test, clean
```

The crates, from the bottom up:

| Crate | Contents |
| --- | --- |
| `fr_math` | glam's vectors, quaternions and matrices, `Dir3`, `Plane`, `Aabb`, rectangles, projections, `Ray3d` |
| `fr_handle`, `fr_time`, `fr_color`, `fr_input` | Typed handles; the frame clock and fixed stepper; `Rgba`; pointer, keyboard and the `InputState` a frame is gathered into |
| `fr_transform`, `fr_image` | `Transform` with `looking_at`; images, samplers and `TextureId` |
| `fr_mesh`, `fr_material`, `fr_light`, `fr_skeleton`, `fr_camera` | Meshes and primitives; PBR material data; lights; `Skeleton::humanoid`; cameras, frusta and the orbit controller |
| `fr_document` | JSON, `Guid`, `PropertyValue`, entity, component and prefab records, component schemas with their dependency rules, the `.scene` and `.prefab` formats, `.meta` sidecars, the asset index, the game schema |
| `fr_assets` | The glTF importer and the typed `AssetLibrary`: models, prefabs and scenes by guid or path, cached |
| `fr_project` | `project.forge`, the project listing, scaffolding from `crates/fr_project/templates`, cargo build steps |
| `fr_physics` | Rigid body physics, a port of Box3D's soft step solver, with the humanoid ragdoll builder |
| `fr_window` | The winit window and event loop |
| `fr_render` | The wgpu device, the draw list, the forward 3D pass and its `Scene` |
| `fr_ui` | The element tree, layout, input routing (clicks, drags, wheel, layers), widgets, dock and theme of the user interface, in games and in the editor |
| `fr_scene` | The entity `Hierarchy`, the `Stage`, the adapters, realization, the physics step and the frame description |
| `fr_engine` | The game-facing crate: `App`, `Behavior`, `ComponentRegistry`, `Runtime`, `Simulation`, `run` and `RunOptions` |
| `fr_authoring` | Headless authoring: documents, commands with undo, gestures, selection, editing rules, the workspace, the launch plan (data crates only) |
| `fr_editor` | The editor and the `forge_editor` binary, described in [editor.md](editor.md) |

Workspace membership is not dependency: the tools sit in `crates/` beside the
engine crates and the dependency direction below holds between them.

## Dependency direction

```text
game   -> fr_engine -> engine crates
editor -> fr_authoring -> engine crates
```

Dependencies never point the other way. winit and wgpu types stay inside
`fr_window` and `fr_render` and do not appear in the public API of `fr_engine`,
`fr_ui` or `fr_input`. No engine crate depends on tooling, and a game depends on
none.

## Layers

A higher layer may use a lower one; a lower layer must not know about a higher
one.

```text
game
fr_engine
fr_scene
fr_ui                 fr_render        fr_window
fr_physics            fr_project, fr_assets
fr_document           data crates (mesh, material, light, skeleton, camera)
fr_math, fr_handle, fr_time, fr_color, fr_input, fr_transform, fr_image
```

- **core** (`fr_math`, `fr_handle`, `fr_time`, `fr_color`, `fr_input`,
  `fr_transform`, `fr_image`) holds primitives, time and input value types. It
  knows nothing of gameplay or graphics APIs. `fr_input::InputState` is the
  frame's gathered pointer, button, scroll and key state a game reads.
- **data** (`fr_mesh`, `fr_material`, `fr_light`, `fr_skeleton`, `fr_camera`)
  are plain data usable without a window or a device.
- **authored data** (`fr_document`, `fr_assets`, `fr_project`) reads content from
  disk and owns the authored data model: records and their validation, the
  versioned JSON formats, the `.meta` sidecars, the asset index, the component
  schemas, the asset library cache and the project tree. It is headless, and it
  is the only part of the engine that opens files at runtime. It knows nothing
  about the scene: turning records into live objects is `fr_scene`'s job. The
  engine creates the `AssetLibrary` for the project's asset root, so no
  filesystem root reaches the `Stage`.
- **physics** (`fr_physics`) owns bodies, shapes, broad phase, contacts, the
  solver, islands and joints. It depends only on `fr_math`, `fr_transform` and
  `fr_skeleton`, is headless and is deterministic.
- **rendering** (`fr_render`) takes a `Scene` and a `DrawList` and draws them. It
  owns the wgpu device, swapchain, pipelines and shaders. It can also render
  offscreen (`Renderer::new_offscreen`, `capture`) and confine the 3D pass to a
  sub-rectangle of the frame (`render_in`), with the UI drawn over it.
- **ui** (`fr_ui`) is the control tree over the draw list: elements, regions and
  layers, captured drags, and generic widgets up to a dock of tabs and splits
  whose layout is plain data (`DockTree`, kept as text). It never knows winit,
  documents or scenes.
- **scene** (`fr_scene`) owns the entity hierarchy, the stage and its adapters,
  realization, the physics step and the description of a frame. It is headless:
  the frame reaches the device through the `RenderResources` trait, which
  `GpuResources` (module `gpu`, with its `ResourceCache`) implements over a
  `Renderer` for the host or an editor.
- **game-facing** (`fr_engine`) wires the layers: the host that drives the
  window, the renderer and the UI, the `Runtime` that owns behaviours and
  structural changes, and `Simulation`, which holds everything that works
  without a window.

Adding a dependency upward, or directly between unrelated subsystems, is an
architectural change. Prefer a narrow data interface at the correct layer.

## The game-facing crate

`fr_engine` is the only engine crate a game names. It exports the `App` trait a
game implements, the `Behavior` trait of gameplay components, the
`ComponentRegistry` a game fills with explicit registration code, the `Runtime`
that loads scenes and runs behaviours, `Simulation`, which holds the scene,
runtime, assets and input of a run and exposes each step of a frame as a method,
and `run`, which builds a window and renderer and drives the frame loop. `Scene`
is `fr_scene::Stage`. Everything in `Simulation` works without a window or a
graphics device, so a test or tool can drive a game step by step. Every other
engine crate a game needs is re-exported under a short name: `math`, `color`,
`time`, `transform`, `input`, `light`, `mesh`, `image`, `material`, `camera`,
`skeleton`, `assets`, `document`, `project`, `physics`, `scene`, `render`, `ui`.

The game's user interface stays `fr_ui`: `App::view` rebuilds the element tree
every frame from the game's own state, and a click comes back as one of the
game's messages through `App::message`.

## The frame

The order is written once, in `fr_engine`'s host. Per fixed step, when
`App::simulates_physics` is true:

1. `App::fixed_update`
2. `Runtime::fixed_update`: the behaviours' fixed updates
3. `Stage::step_physics`: kinematic and static bodies take their entity's pose,
   the world steps, every awake dynamic body writes its pose back to its entity

Per rendered frame:

1. `App::update`
2. `Runtime::update`: activation refresh, then the behaviours' updates
3. the user interface: `App::view` is built and painted into the draw list
4. `Runtime::structural_boundary`: scene replacement, or the destruction of
   queued entities and then the queued prefab spawns
5. `Stage::resolve_transforms`
6. `Stage::apply_transform_interpolation`
7. `Stage::build_render_scene`, which borrows the stage and describes the frame
8. `Renderer::render`

Nothing between building the render scene and the draw may touch the stage. The
host owns the frame clock, the fixed stepper, the first failure and the
frame limit; a failing callback ends the loop after the frame and the run
reports it.

## The render scene boundary

`Stage::build_render_scene` fills an `fr_render::Scene`: the current camera, the
ambient light, the lights of active entities and every drawn instance. The stage
is headless and holds no device handle, so meshes, materials and models are
asked for through `RenderResources`:

| Call | What the host does |
| --- | --- |
| `model(guid)` | Loads the glTF through the `AssetLibrary` and uploads it once |
| `primitive(Primitive)` | Generates and uploads a cube, sphere or capsule once |
| `material(&MaterialSpec)` | Creates the material of a specification once |
| `default_material()` | The renderer's default material |

Cameras, lights and renderers are drawn at the entity's *render* transform, the
global transform with the interpolation of simulated bodies applied.

## The transform resolution boundary

Writing an entity's transform marks it and its subtree stale instead of walking
it. `Stage::resolve_transforms` pays that debt and the host calls it after the
structural boundary and before the frame is described, for the same reason the
boundary sits there: everything that can move an entity has already run. That
single call is a placement, not a requirement: every accessor that hands out a
global pose (`Stage::global_transform`), the adapters at realization and
`Stage::step_physics` resolve first, and resolving twice costs nothing.

`Stage::apply_transform_interpolation` runs immediately after. It sets the
render transform of every entity: a dynamic body at the pose `alpha` of the way
between its last two steps, everything else following its parent. It writes no
local transform, no cached global and no physics body, which keeps interpolation
off the simulation. A paused game draws with `alpha` one.

## The destroy queue boundary

`Stage::queue_destroy_entity` records a handle and changes nothing else.
`Stage::flush_destroy_queue` is the one place in the frame where queued
entities are destroyed, with everything below them and everything their adapters
made, and `Runtime::structural_boundary` calls it. The boundary tears down the
behaviours of dying entities first, so their `destroy` callbacks still see the
entity, flushes the queue, then realizes the queued spawns. A scene replacement
queued with `Runtime::queue_scene` or `Runtime::reload_scene` happens at the
same boundary, and instead of the rest of it. Destruction requested from a
callback waits for the next boundary and is never flushed recursively.

Immediate `Stage::destroy_entity` is for setup and teardown, where the caller
knows nothing refers to the subtree.

## Lifetime and error rules

- Resource ownership is explicit and deterministic. Prefer RAII and make partial
  initialization safe.
- The behaviours are torn down before the simulation is dropped, and the window
  and renderer end with the loop.
- Setup runs before a window exists: the project opens, the startup scene loads
  and `App::start` runs first, so a failure there never shows a window.
- Initialization failures log `forge: <message>` and return a nonzero exit code.
- `WGPU_VALIDATION=1` turns on wgpu's validation in any build.
- A realization that fails destroys exactly the entities it created and leaves
  the stage, and its settings, as they were. A callback's own side effects
  cannot be rolled back.
- Failures are values: `DocumentError`, `AssetError`, `ProjectError`,
  `SceneError` and `EngineError`, each saying what failed and where. Nothing
  panics on bad input.
- Versions detect mismatch and never imply compatibility; see
  [authoring.md](authoring.md#versions).
