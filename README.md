# forge

A data-driven Rust game engine built on winit and wgpu. A game is a project: a directory of glTF models, prefabs and scenes (`.scene` and `.prefab` JSON documents with `.meta` sidecars) plus its own Cargo project that depends on `fr_engine`. See [AGENTS.md](AGENTS.md) for the layout and rules and [docs](docs/) for the guides.

```sh
make build
make -C ~/Documents/forge/demo run
make run ARGS=~/Documents/forge/demo   # the editor
```

The demo game lives outside this repository: a lit PBR scene with shadows, a pile of rigid bodies and ragdolls, all authored in `assets/scenes/demo.scene`, with an orbit camera, a UI panel and prefab spawning written as behaviours.

`fr_physics` is a Rust port of the soft step solver of Box3D: spheres, capsules and boxes (convex hulls), speculative contacts with warm starting, relaxation and restitution, a dynamic AABB tree, islands with sleeping, continuous collision against static shapes, spherical, revolute, weld and distance joints, sensors, ray casts and a humanoid ragdoll builder. It is scalar and single threaded, and deterministic bit for bit.

The editor, `forge_editor`, opens a project in a dock of hierarchy, project, inspector, console and a live viewport with a transform gizmo, edits scenes and prefabs with undo, and launches the game as a separate process with Play. `make run ARGS=~/Documents/forge/demo` starts it; see [docs/editor.md](docs/editor.md).
