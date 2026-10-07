# Getting started

## Requirements

- Rust 1.88 or newer with `cargo`
- GNU Make
- A GPU driver wgpu can use: Vulkan on Linux and Windows, DirectX 12 as the
  fallback on Windows, Metal on macOS

The renderer is wgpu and the window winit, both built by Cargo with the rest of
the workspace in `crates/`; the first build downloads them and the other crates
`Cargo.lock` pins. Shaders are WGSL inside the `fr_render` crate and compiled by
wgpu when a pipeline is built, so no shader compiler or graphics SDK is needed.

`WGPU_BACKEND` (`vulkan`, `dx12`, `metal`) limits the backends wgpu may pick.
`WGPU_VALIDATION=1` turns on wgpu's validation and, on Vulkan, the Khronos
validation layer when it is installed.

On Linux, winit opens a Wayland or an X11 window, loading the client libraries at
run time, and wgpu loads the Vulkan loader the same way, so nothing beyond the
driver has to be installed to build.

## Commands

In the engine repository:

```sh
make build  # cargo build --workspace --all-targets
make fmt    # cargo fmt --all
make lint   # rustfmt check and clippy with warnings denied
make test   # cargo test --workspace
make clean  # cargo clean
make run    # cargo run --release -p fr_editor; ARGS=<project> opens a project
make debug  # the same, for a debug editor
make env    # cargo install the editor (forge_editor)
```

There is no game in the engine repository. A game is a project beside it, with
its own Makefile; `make -C <project> run` runs it in release and `make -C
<project> debug` in debug. The demo lives at `~/Documents/forge/demo`:

```sh
make -C ~/Documents/forge/demo run
make -C ~/Documents/forge/demo run ARGS="--scene scenes/demo.scene"
```

## A project

A project is a directory with a `project.forge` beside an asset root and the
game's own Cargo project:

```text
project.forge             name, asset root, game executable, startup scene
Cargo.toml, src/          the game: a Cargo project depending on fr_engine
assets/                   the asset root
  models/ground.gltf      glTF models, each with a .meta sidecar
  prefabs/box.prefab      reusable hierarchies
  scenes/demo.scene       levels
```

`project.forge` is `key = value` lines:

```text
asset_root = assets
game_executable = target/release/forge_demo
name = demo
startup_scene = scenes/demo.scene
```

A game started in the project directory, which is where `make run` starts it,
finds `project.forge`, opens the asset root and loads the startup scene. The
command line overrides it: `--project <dir>` starts from another directory,
`--scene <path>` names the scene to start in, relative to the asset root,
`--frames <n>` ends the run after `n` frames, and `--width` and `--height` size
the window. Unknown arguments are left for the game.

Make a new project from the templates with `fr_project::create_project`, which
needs no command line:

```rust
use std::path::Path;

use fr_engine::project::{create_project, engine_crate_dir};

fn main() {
    let engine = engine_crate_dir();
    match create_project(Path::new("/path/to/my_game"), &engine) {
        Ok(project) => println!("created {}", project.settings().name),
        Err(error) => eprintln!("{error}"),
    }
}
```

It writes a `Cargo.toml` depending on `fr_engine` by path, a `src/main.rs`, a
`src/behaviours/mod.rs`, a `project.forge`, a `.gitignore` and an
`assets/scenes/main.scene` with a camera and a sun, each asset with its `.meta`.

## A minimal game

A game implements `App` and hands it, a `ComponentRegistry` and `RunOptions` to
`run`. `start` runs once after the startup scene is loaded; `update` runs once per
rendered frame. Both receive an `AppContext` with the `Scene`, the `Runtime`, the
`AssetLibrary` and the `InputState`. The world itself comes from the scene
document, so the game below builds nothing in code.

```toml
[package]
name = "minimal"
version = "0.1.0"
edition = "2024"

[workspace]

[dependencies]
fr_engine = { path = "path/to/forge/crates/fr_engine" }
```

```rust
use std::process::ExitCode;

use fr_engine::prelude::*;
use fr_engine::{export_schema_if_requested, parse_run_options, run};

/// A game that loads the startup scene and leaves what happens in it to the
/// behaviours.
struct Minimal;

impl App for Minimal {
    type Message = ();

    /// Called once after the startup scene is loaded.
    fn start(&mut self, _context: &mut AppContext<'_>) -> EngineResult {
        Ok(())
    }

    /// Called once per rendered frame, before the behaviours' update.
    fn update(&mut self, _context: &mut AppContext<'_>, _frame: &FrameInfo) -> EngineResult {
        Ok(())
    }
}

/// Registers behaviours, writes the schema on request and runs the game.
fn main() -> ExitCode {
    let registry = ComponentRegistry::new();
    let arguments: Vec<String> = std::env::args_os()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    match export_schema_if_requested(&registry, &arguments) {
        Ok(true) => return ExitCode::SUCCESS,
        Ok(false) => {}
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::FAILURE;
        }
    }
    let options = parse_run_options(std::env::args_os());
    run(&mut Minimal, registry, &options)
}
```

Run it in a directory with a `project.forge`, or point it at one with `--project`.
`type Message` is what the controls of `App::view`, the game's `fr_ui` tree, send
back through `App::message`; a game with no interface uses `()`.

A game gets behaviour by registering component types, each a schema and a factory,
before it calls `run`:

```rust
use fr_engine::prelude::*;

/// Turns its entity about the vertical axis.
#[derive(Default)]
struct Turner {
    /// Radians per second.
    speed: f32,
}

impl Behavior for Turner {
    /// Reads the authored speed.
    fn apply_property(
        &mut self,
        _context: &mut BehaviorContext<'_>,
        key: &str,
        value: &PropertyValue,
    ) -> EngineResult {
        if key == "speed" {
            self.speed = value.as_float().unwrap_or(0.0);
        }
        Ok(())
    }

    /// Turns the entity.
    fn update(&mut self, context: &mut BehaviorContext<'_>, frame: &FrameInfo) -> EngineResult {
        let entity = context.entity();
        if let Some(transform) = context.scene.transform(entity) {
            let turn = Quat::from_rotation_y(self.speed * frame.delta_seconds);
            context
                .scene
                .set_transform(entity, transform.with_rotation(turn * transform.rotation));
        }
        Ok(())
    }
}

/// Registers the game's component types.
fn register(registry: &mut ComponentRegistry) {
    let schema = ComponentSchema {
        key: "game.turner".to_owned(),
        label: "Turner".to_owned(),
        properties: vec![PropertySchema::float("speed", 1.0)],
        ..ComponentSchema::default()
    };
    registry.add_behavior(schema, || Box::new(Turner::default()));
}
```

A scene then carries a component of type `game.turner` on any entity. The demo at
`~/Documents/forge/demo` is a complete game: its `src/behaviours/` registers an
orbit camera and a spinner, `src/panel.rs` is the interface and `src/game.rs`
spawns prefabs and resets the scene.

## Running without a window

`fr_engine::Simulation` holds the scene, runtime, assets and input of a run and
has one method per step of a frame, so a tool or test can load a scene, step the
physics until the bodies rest and read the result without a window or a device:

```rust
let mut simulation = Simulation::open(registry, &options)?;
simulation.load_scene(Path::new("scenes/demo.scene"))?;
simulation.fixed_update(&mut app, &FixedFrameInfo { index: 0, delta_seconds: 1.0 / 60.0 })?;
```

## Where to go next

- [Architecture](architecture.md) explains the layers, the frame order and the
  boundaries.
- [Authoring](authoring.md) explains entities, components, scenes, prefabs,
  identity, saving and the behaviour lifecycle.

## The editor

`make run ARGS=~/Documents/forge/demo` opens a project in the editor; plain
`make run` reopens the last project, or shows the project picker, pre-filled
with `~/Documents/forge`, until one has been opened. See
[editor.md](editor.md).
