# forge

A general-purpose Rust game engine built on winit and wgpu. See [AGENTS.md](AGENTS.md) for the layout and rules.

```sh
make run
```

The demo: lit PBR scene with shadows, optional glTF (`make run ARGS=path/to/model.glb`), and one ragdoll simulated by `fr_physics`. Hold the right mouse button to look around and fly (W A S D, E or space up, Q or ctrl down, shift faster, wheel for speed); press the left button on the ragdoll to grab and drag it, with the wheel moving it towards or away from you. The panel resets the ragdoll and sets the strength, damping and torque limit of its joint motors.

`fr_physics` is a Rust port of the soft step solver of Box3D: spheres, capsules and boxes (convex hulls), speculative contacts with warm starting, relaxation and restitution, a dynamic AABB tree, islands with sleeping, continuous collision against static shapes, spherical, revolute, weld, distance and mouse joints, sensors, ray casts, joint motors (torque limited springs towards a target pose) and data driven rigs built on a transform hierarchy, with a humanoid template. It is scalar and single threaded, and deterministic bit for bit.
