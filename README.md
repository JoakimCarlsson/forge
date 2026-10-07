# forge

A general-purpose Rust game engine built on winit and wgpu. See [AGENTS.md](AGENTS.md) for the layout and rules.

```sh
make run
```

The demo: lit PBR scene with shadows, optional glTF (`make run ARGS=path/to/model.glb`), and ragdolls dropped onto a pile of rigid bodies simulated by `fr_physics`.

`fr_physics` is a Rust port of the soft step solver of Box3D: spheres, capsules and boxes (convex hulls), speculative contacts with warm starting, relaxation and restitution, a dynamic AABB tree, islands with sleeping, continuous collision against static shapes, spherical, revolute, weld and distance joints, sensors, ray casts and a humanoid ragdoll builder. It is scalar and single threaded, and deterministic bit for bit.
