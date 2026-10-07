# Forge

A general-purpose Rust game engine on winit and wgpu. Keep the engine reusable:
engine code never contains game-specific rules, content or scenes.

## Commands

```sh
make run   # Run the example (EXAMPLE=clear by default) in release
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

- `fr_core`: frame timing and shared value types; no window or graphics dependency.
- `fr_window`: the winit window and event loop.
- `fr_render`: the wgpu device, swapchain and renderer.
- `fr_engine`: the crate a game depends on (`App`, `run`); examples live in `crates/fr_engine/examples`.

## Rules

- Dependencies point downwards only: `fr_engine -> fr_render, fr_window, fr_core`. A lower crate never knows a higher one.
- winit and wgpu types never appear in the public API of `fr_engine`.
- `fr_core` stays usable without a window or graphics device, which keeps its tests headless.
- Initialization failures return an error and the binary exits nonzero.
- Do not add speculative subsystems before their contracts are defined. Keep changes scoped.

## Conventions

- Document every item, private ones included, with a rustdoc comment in the style of the standard library. Function bodies contain no comments.
- Fix clippy diagnostics rather than allowing lints locally; `make lint` stays clean.
- Return `Result` for failure; do not panic on bad input or a failing device.
- Write build output to the git-ignored `build/`, never `/tmp`.
