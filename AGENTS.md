# Forge

A general-purpose Rust game engine on winit and wgpu. Keep the engine reusable:
engine code never contains game-specific rules, content or scenes.

## Commands

```sh
make run   # Run the example (EXAMPLE=model by default; clear and ui also exist) in release; ARGS passes arguments, e.g. make run EXAMPLE=model ARGS=path/to/model.glb
make debug # Run the example in debug
make build # cargo build --workspace --all-targets
make fmt   # cargo fmt --all
make lint  # cargo fmt --all --check and cargo clippy --workspace --all-targets -- -D warnings
make test  # cargo test --workspace
```

## Structure

```text
Cargo.toml   Workspace root: members are crates/*; rustfmt.toml and clippy.toml sit beside it
crates/      The workspace members, all prefixed fr_
```

- `fr_core`: frame timing and shared value types; no window or graphics dependency. Owns the one set of math types (glam's `Vec3`, `Quat`, `Mat4`, plus `Transform`, `look_at`, `perspective`, `orthographic`), the light types (`DirectionalLight`, `PointLight`, `SpotLight`, `Light`) and the `MeshId`/`MaterialId`/`TextureId` handles.
- `fr_assets`: CPU-side asset data (`MeshData`, `MaterialData`, `ImageData`, `ModelData`), the built-in `cube`/`sphere`/`plane` and the glTF/GLB importer (`load_gltf`, including `KHR_lights_punctual`). Depends on `fr_core` only; never touches a device.
- `fr_window`: the winit window and event loop; forwards pointer, scroll, key, text and scale-factor events as crate-owned types.
- `fr_render`: the wgpu device, swapchain, glyph atlas, text shaping (`TextSystem`), the quad and glyph pipelines, and the draw list (`DrawList`, `Quad`, `Rgba`, `Rect`) a frame is submitted as. Also the forward 3D pass drawn under the UI in the same submission: uploads of assets to handles, a `Scene` (camera, lights, mesh instances), metallic-roughness PBR, point/spot/directional lights, cascaded sun shadows and spot/point shadow maps. Present mode defaults to uncapped (`Renderer::set_vsync`).
- `fr_ui`: the element tree, layout pass, hit testing, focus and input routing over `fr_render`'s draw list; style, the one fixed `Theme` (tokens only, no switching), SVG icons (`IconName`, `icon`, `icon_button`; artwork in `assets/icons` with its `LICENSES`), `Div`, `Text` and the widgets.
- `fr_engine`: the crate a game depends on (`App`, `run`, `Assets` with `load_gltf`); `App::scene` describes the 3D frame, `App::vsync` chooses vsync; re-exports `fr_ui` as `ui` and `fr_assets` as `assets`. Examples live in `crates/fr_engine/examples` (`clear`, `ui`, `model`).

## Rules

- Dependencies point downwards only: `fr_engine -> fr_ui, fr_render, fr_assets, fr_window, fr_core`, `fr_ui -> fr_render`, `fr_render -> fr_assets, fr_core`, and `fr_assets -> fr_core`. `fr_ui` never knows winit. A lower crate never knows a higher one.
- winit and wgpu types never appear in the public API of `fr_engine`, `fr_ui`, or the input types of `fr_window`. wgpu stays inside `fr_render`.
- `fr_core` stays usable without a window or graphics device, which keeps its tests headless.
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
