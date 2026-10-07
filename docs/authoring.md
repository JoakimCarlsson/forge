# Authoring model

Scenes contain entities, entities have components, and prefabs are reusable
entity hierarchies. Game behaviour is compiled Rust in the game's own
executable; the data is edited without running it. Models are glTF.

This page is the decision record for that model: what each piece owns, how it is
identified, how it is stored and when it runs. The layer rules it sits inside
are in [architecture.md](architecture.md).

## Documents, assets and files

| File | Holds | Read by |
| --- | --- | --- |
| `project.forge` | Project name, asset root, game executable, startup scene | The game, tooling |
| `<name>.scene` | One level: entity records, prefab instance records, scene settings | The game |
| `<name>.prefab` | One reusable hierarchy: entity records and prefab instance records | Instantiated into scenes and prefabs, spawned at runtime |
| `<name>.gltf`, `<name>.glb` | Meshes, materials, textures and lights | Referenced by a `forge.mesh_renderer` component |
| `<asset>.meta` | The asset's identity | Everything that resolves a reference |

`.scene`, `.prefab` and `.meta` are UTF-8 JSON with stable field ordering and
deterministic formatting: keys in insertion order, two spaces of indentation,
arrays of scalars on one line and a final newline. `project.forge` is a plain
`key = value` file. `fr_document` holds the value types (`EntityDocument`,
`SceneAsset`, `PrefabAsset`, `SceneSettings`, `AssetMeta`) and the component
schema registry. It parses and writes the JSON, validates documents against the
registered component types, reads and writes `.meta` sidecars, replaces files
atomically and scans a project for assets. A file whose text is not UTF-8 is
rejected with the byte offset.

`.meta` sidecars are committed project data, not tool preference, and live beside
the asset with `.meta` appended: `models/ground.gltf.meta`.

A scene file looks like this, abridged:

```json
{
  "format": "forge.scene",
  "format_version": 1,
  "settings": {
    "clear_color": [0.45, 0.6, 0.8, 1.0],
    "ambient": { "sky_color": [0.55, 0.7, 1.0], "ground_color": [0.35, 0.3, 0.25], "intensity": 0.2 },
    "physics": { "gravity": [0.0, -10.0, 0.0], "sub_steps": 4 }
  },
  "entities": [
    {
      "id": "c961c37ba2223efe236a87ebecbf1f63",
      "name": "Camera",
      "parent": "00000000000000000000000000000000",
      "order": 0,
      "active": true,
      "transform": { "translation": [2.73, 4.11, 5.0], "rotation": [-0.24, 0.24, 0.06, 0.94], "scale": [1.0, 1.0, 1.0] },
      "groups": [],
      "components": [
        {
          "id": "d0d19c0237cad7aafeb1e99534536bd1",
          "type": "forge.camera",
          "schema_version": 1,
          "enabled": true,
          "properties": [{ "key": "current", "type": "bool", "value": true }]
        }
      ]
    }
  ],
  "instances": []
}
```

(The real file writes every object across lines.) `clear_color` is sRGB, like
every colour of the user interface; the colours of components are linear.

## Identity

`Guid` is a random 128-bit identifier serialised as 32 lowercase hex digits. The
same type identifies assets, authored entities, component records and prefab
instances. Runtime handles stay generational and are never serialised.

- An asset's `Guid` lives in its `.meta` sidecar. References store that id and a
  `last_known_path` used only for diagnostics; a missing id is an unresolved
  reference, never a silent path match.
- `fr_document::AssetIndex` builds id-to-path from the sidecars under the asset
  root. The index is disposable and rebuilt by scanning. Two assets claiming one
  id are reported with both paths and the later one is not indexed.
- `AssetLibrary::guid_of(path)` and `AssetLibrary::reference(path)` turn a path
  relative to the asset root into the id or the reference a game needs.
- Entity references are an instance chain plus an entity id,
  `EntityReference { instances, entity }`. Component references add a component
  id. A reference is read inside the prefab instances that contain the component
  that carries it, so each instance of a prefab resolves a reference to its own
  entity.
- Runtime handles are generational: a handle from an unloaded scene, or to a
  destroyed entity, never resolves again. An entity made from a prefab shares the
  authored id of the prefab's record with every other instance of it, so the
  authored id is not a runtime key.

## Records

```text
EntityRecord          id, name, transform, parent, order, active, groups[], components[]
ComponentRecord       id, type, schema_version, enabled, properties[]
PrefabInstanceRecord  id, name, transform, parent, order, active, prefab, overrides[]
PropertyOverride      entity, component, property, value
```

Every entity has exactly one transform and one name, owned by the entity record.
The schema registry also describes them as the types `forge.transform` and
`forge.name`, marked `intrinsic` and not removable, so tooling can show them as
sections like any component; a document that carries either as a component
record is rejected. Hierarchy parenting is spatial composition: a simulated body
keeps its own authority.

A property value is self-describing in the file (`{"key": …, "type": …,
"value": …}`), so a component whose schema is temporarily unavailable still
round-trips instead of being erased. Supported types are `bool`, `int`, `float`,
`string`, `enum` (stable option keys), `vec3`, `color` (linear RGBA), `rotation`
(a quaternion as `[x, y, z, w]`), `transform`, `asset`, `entity`, `component`
and `array`s of those. A transform is `{"translation", "rotation", "scale"}`.

### Built-in component types

| Type key | Purpose | Rules |
| --- | --- | --- |
| `forge.transform`, `forge.name` | The entity's own placement and label | Intrinsic: never a component record |
| `forge.mesh_renderer` | Draws a glTF `model`, or a `primitive` (`cube`, `sphere` of diameter one, `capsule` with `radius` and core `length`) when no model is set; `override_material` replaces the materials with `base_color`, `metallic`, `roughness`, `emissive`, `emissive_intensity` and `double_sided` (a base colour with an alpha below one blends) | One per entity; the entity's scale scales the mesh |
| `forge.camera` | `projection` (perspective or orthographic), `fov_y_degrees`, `orthographic_height`, `near`, `far`, `exposure`, `current` | One per entity; looks along the entity's forward axis, negative Z, with its Y as up; the first camera that asks becomes the scene's current camera |
| `forge.light` | `kind` (`directional`, `point`, `spot`), `color`, `intensity`, `range`, `inner_angle_degrees`, `outer_angle_degrees`, `cast_shadows`, `shadow_distance` | One per entity; directional and spot lights shine along the entity's forward axis, point and spot lights sit at the entity; units are those of `fr_light` |
| `forge.rigid_body` | `body_type` (`static`, `kinematic`, `dynamic`), `linear_damping`, `angular_damping`, `gravity_scale` | One per entity; conflicts with `forge.ragdoll`; mass comes from the densities of its colliders |
| `forge.collider` | `shape` (`sphere`, `capsule`, `box`), `radius`, `half_height`, `half_extents`, `offset`, `density`, `friction`, `restitution`, `rolling_resistance`, `sensor` | Several per entity; requires `forge.rigid_body`; its size is its own and does not read the entity's scale |
| `forge.ragdoll` | A humanoid of capsule and box bodies joined by limited joints, built by `fr_physics` at the entity's placement: `friction_torque`, `joint_hertz`, `joint_damping_ratio`, `density`, `color`, `roughness`, `show_bones`, `bone_color` | One per entity; conflicts with `forge.rigid_body`; each ragdoll gets its own collision group; draws its own limbs and bone markers |

Each type declares a stable namespaced key, a schema version, stable property
keys, defaults, multiplicity and dependency rules in the component registry,
whose built-in schemas are the Rust table in
`crates/fr_document/src/builtin_schema.rs`. The data and runtime layers validate
those rules, not only a future inspector: `validate_document` reports a
duplicated single component, a missing requirement, a conflict, a property of
the wrong type, a schema version mismatch, an unset or reused id, a missing
parent and a parent cycle as errors, and an unknown component type or property
as warnings. Registered defaults fill newly created components
(`components::make_component`) and any property a record lacks when it is read.

Game behaviours are registered the same way, with
`ComponentRegistry::add_behavior`, from the game's executable. Their schema is
marked as the game's own, and they are single-instance unless their schema opts
into several. Behaviours are the *Behavior* class of component: there is no
generic behaviour type, each game type has its own key and schema.

`Scene settings` are part of the scene file, not of any entity: the clear colour,
the ambient light and the physics world (`gravity`, `sub_steps`).

## Versions

The container format version (`format_version` of `forge.scene`, `forge.prefab`,
`forge.meta` and `forge.game_schema`) and each component's `schema_version` are
separate. Only the current versions are accepted. Versions detect mismatch; they
do not promise compatibility. There are no old readers, field aliases, coercions
or migration chains: an incompatible file is rejected with a diagnostic naming
the file, the version found and the version required, and breaking schema
changes mean recreating the affected data.

```text
forge.scene format version 2 is not supported; version 1 is required
component forge.camera has schema version 5 but 1 is required
```

Opaque data for a component type whose schema is unavailable is preserved across
a save, reported as a warning at load, and never deleted. That does not make it
runnable: the realization keeps it on the entity as inert data.

## Realization

`Stage::realize_scene` and `Stage::realize_prefab` turn records into runtime
objects through the registered adapters. They are the single path for the scene
a game starts in, a scene that replaces it and a prefab spawned during play. The
walk lives in `crates/fr_scene/src/realize.rs`; the adapters live in
`crates/fr_scene/src/adapters/`:

1. Validate the document against the registered schemas; errors fail the load.
2. Apply the scene settings.
3. Create entities in parent-first order, siblings by their `order`.
4. Realize each prefab instance under its own entity, applying the instance's
   overrides to the prefab's entities, up to 32 levels deep.
5. Create each entity's components through the adapters, in adapter order: camera,
   light, mesh renderer, rigid body, collider, ragdoll. A body is therefore there
   before its colliders.
6. Collect the components of game types as behaviours, in entity creation order
   then component order.

Every entity created by one walk is tracked, so an asset or component failure
destroys exactly what that walk created and leaves the stage as it was.
Components that are disabled in the record are not realized.

An override replaces a value only when it has the property's type; it addresses
the entity of the prefab by entity id, the component by component id and the
property by key, and `name`, `transform` and `active` with an unset component id.
Overrides apply to the instance's own prefab, not to the prefabs nested inside
it.

## Behaviour lifecycle

```text
construct -> apply properties -> resolve references -> validate batch
          -> initialize (once) -> enable/disable follow activation
          -> fixed_update / update while initialized and enabled
          -> disable -> destroy
```

The stages are the `Behavior` trait's methods: `apply_property`, `apply_entity`
and `apply_component`, `validate`, `initialize`, `enable` and `disable`,
`fixed_update`, `update` and `destroy`, each taking a `BehaviorContext` that
carries the scene, the runtime, the assets and the input, and returning
`EngineResult`. Every method has a default that does nothing. Constructors, the
registered factories, have no scene side effects. A behaviour receives every
property of its schema, the record's value or the schema's default. Initialization
does not imply that another behaviour has initialized; cross-object work begins
after the batch barrier. There is no editable execution priority: behaviours run
in the order they were created.

A behaviour is active while its own flag and its entity's activity in the
hierarchy are both on. `Runtime::set_kind_enabled(key, false)` pauses every
behaviour of one component type without touching its activation, and `App`
handles anything that coordinates behaviours with each other.

Per fixed step: the application's fixed update, the behaviours' fixed updates,
physics. Per rendered frame: the application's update, the behaviours' updates,
the user interface, the structural boundary, transform resolution, interpolation,
the frame description and the draw. Behaviour fixed updates stay inside the
simulation gate: a game that returns false from `App::simulates_physics` runs
neither them nor the physics.

At the structural boundary the runtime tears down the behaviours of dying
entities, flushes the destroy queue, then realizes additions. A callback that
returns an `Err` rolls a scene load back, and from an update ends the frame loop
with the error reported.

Runtime prefab spawning is two-stage: submit the asset, placement and overrides
as a `SpawnRequest` to `Runtime::spawn`, receive a `SpawnToken`, and read
`Runtime::spawn_result` for the root once the batch is realized and initialized at
the boundary. The prefab's entities go under a new root entity named and placed
by the request. A failed spawn is reported in its result and leaves the scene as
it was. Newly spawned behaviours first update on the following frame. The scene
a game starts in is realized synchronously before the frame loop starts.

## Scene transitions and session state

Scene replacement is queued for the structural boundary, with `Runtime::queue_scene`
(a path relative to the asset root), `Runtime::queue_scene_asset` or
`Runtime::reload_scene`. The new scene is read and validated when it is queued, so
invalid input leaves the running scene alone. At the boundary the old scene is torn
down and the new one realized; a realization failure ends the run, there is no
double residency of two worlds.

Session state belongs to the application object, which outlives scenes. There are
no persistent scene entities and no automatic cross-scene reference repair.

## Saving

A save writes the complete validated document to a uniquely named temporary file
in the destination directory, flushes and closes it, then replaces the target with
a rename (`fr_document::write_file`). The original is never truncated first, and
a failed write removes the temporary file. An atomic replacement is not a
power-loss guarantee. Components of unregistered types, and properties a schema
does not list, are carried through reading and writing unchanged, because their
values describe their own types; a document the engine wrote reads back and
writes again as the same text.

## External Rust projects

A game is an external Cargo project that depends on the `fr_engine` crate by
path and registers its behaviours in its own executable. Its `main` builds a
`ComponentRegistry`, registers the behaviours, and calls
`export_schema_if_requested(&registry, &arguments)` before any window or device
exists: with `--forge-export-schema <path>` it writes the game schema (the game's
types and a fingerprint of every registered type) and returns `true`, and `main`
exits without constructing behaviours or starting gameplay. Otherwise it hands
the registry to `run`.

Registration is explicit: a behaviour module has a `register` function, and
`src/behaviours/mod.rs` lists every module and calls each one from
`register_all`. Nothing registers itself as a side effect of being linked.
Editing serialised property values never needs a rebuild; changing Rust does.

`fr_project::create_project` writes a new game from the templates in
`crates/fr_project/templates` (`Cargo.toml.in`, `main.rs.in`, `behaviours.rs.in`,
`gitignore.in`): a `Cargo.toml` depending on `fr_engine`, a `src/main.rs`, a
behaviour module, `project.forge`, an ignore file and a first scene with a camera
and a sun under `assets/scenes/`. `fr_project::build_steps` lists the cargo steps
that build it.

A game finds its project from the directory it is started in, or from
`--project <dir>`, and starts in the scene `--scene <path>` names, relative to the
asset root, or else in the project's `startup_scene`. A directory without a
`project.forge` is itself the asset root and has no startup scene.

## Worked example: a door

1. Create an entity named `Door`. It gets an id and a transform.
2. Add a `forge.mesh_renderer` referencing `models/door.gltf` by its guid, and a
   `forge.rigid_body` set to `kinematic` with a `forge.collider` box.
3. Add a child entity `Panel` with its own transform and mesh renderer.
4. Register `game.door` in the game with `add_behavior`. Its schema exposes
   `open_speed` (float, default `1.5`) and `panel` (entity reference). Add the
   component to `Door` and set `panel` to the `Panel` child. The reference stores
   an empty instance chain and `Panel`'s entity id, because both live in the same
   document.
5. Save as `prefabs/door.prefab`. A `.meta` sidecar gives the prefab its asset id.
6. Place the prefab in `scenes/hall.scene` twice. Each placement is a
   `PrefabInstanceRecord` with its own instance id and transform.
7. Set `open_speed` to `0.8` on the second instance. That writes one
   `PropertyOverride` keyed by the door entity id, the behaviour component id and
   the property key `open_speed`; the first instance is untouched.
8. Run the game with `--scene scenes/hall.scene`. Each instance resolves `panel` to
   its own `Panel` entity because resolution runs inside that instance's chain.
9. Change the Rust and rebuild: both authored values survive because they are
   keyed by stable ids.
