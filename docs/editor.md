# Editor

The editor is a live view of a scene that can be edited while you look at it.
Whatever changes in the document, the viewport shows on the next frame. It is
the Rust crate `crates/fr_editor`, a library and the `forge_editor` binary,
built on `fr_authoring` (documents, commands, history, selection, rules and the
launch plan), `fr_ui` (the toolkit), `fr_scene` and `fr_render` (the live
preview) and `fr_window` (the window). It runs its own frame loop. The
dependency direction is `editor -> fr_authoring -> engine crates`, never the
reverse, and the editor links no game and knows nothing about one.

## Running it

```sh
make run                                   # cargo run --release -p fr_editor
make run ARGS=~/Documents/forge/demo       # open a project
make debug                                 # a debug editor
make build                                 # build the workspace
make env                                   # cargo install forge_editor
forge_editor path/to/project
```

The project is the first bare argument or `--project <path>`. Without one the
editor reopens the last project it opened; until a project has been opened it
shows the project picker, pre-filled with `~/Documents/forge`, the folder that
holds forge projects, and lists its subfolders that contain a `project.forge`.
The picker also lists recent projects, opens any path and creates a new
project from the scaffold.

The editor can also draw without a window or a pointer, which is how its
interface is checked on a machine without a display. A capture renders
offscreen, writes a PNG and exits:

```sh
forge_editor path/to/project --capture build/editor.png --frames 6 --size 1600x900
```

| Option | Effect |
| --- | --- |
| `--capture <png>` | Render offscreen and write the last frame to a PNG instead of opening a window |
| `--frames <n>` | Frames to run before the capture, 6 by default |
| `--size <W>x<H>` | Size of the target in pixels, 1600x900 by default |
| `--scale <factor>` | Display scale factor, 1 by default |
| `--scene <path>` | Scene to open, relative to the asset root or absolute |
| `--select <name>` | Select the entity with that name once the scene is open |
| `--tool <name>` | Tool to start with: `select`, `move`, `rotate` or `scale` |
| `--picker` | Show the project picker even when a project is known |
| `--project <path>` | The project, as an alternative to the first bare argument |

Failures print `forge: <message>` and exit nonzero.

## Projects

A project is a directory with a `project.forge`, an asset root of models,
prefabs and scenes and, usually, the game's own Cargo project (see
[getting-started.md](getting-started.md)).

| Path | Contents | Committed |
| --- | --- | --- |
| `<project>/project.forge` | Name, asset root, game executable and startup scene | Yes |
| `<project>/**/*.meta` | One sidecar per scene, prefab and model holding its identity | Yes |
| `<project>/.forge-editor/` | Per-project editor state: the open documents and the current one. It carries its own `.gitignore`, and the scaffold ignores it too | No |
| The per-user config directory (`forge-editor/`) | `editor.ini` (last project, recent projects, window size, projects root) and `layout.txt`, the dock layout | Not in the project at all |

Nothing the editor owns for its own sake is written into a project's source
tree.

## Layers

Dependencies point down; nothing below knows about anything above it.

```text
panels          interface only: read state, send commands, never mutate
   |
viewport        camera, picking, tools, gizmo drawing
   |
preview         document -> live Stage, authored identity <-> entity handle
   |
commands        fr_authoring: mutations with undo, gestures, selection, rules
   |
document        the edited data, its path and dirty state
```

The bottom two layers hold most of the logic that can be wrong and none of them
needs a window or a device, so they live in `fr_authoring`, which depends only
on `fr_document`, `fr_project`, `fr_math` and `fr_transform`.

```text
crates/fr_editor/src/
  main.rs, options.rs   the binary and its arguments
  app/                  EditorState, Message, the loop and window handler, headless
                        capture, menus, layout, picker, shortcuts, project opening
  panels/               hierarchy, project, inspector, console, the panel registry
  viewport/             editor camera, picking, transform gizmo, grid, overlay drawing
  preview/              the document realized into a stage, never simulated
  subsystems/           one folder per engine subsystem being authored
  run/                  building and launching the game as a separate process
  settings/             per-user settings, per-project state, project discovery
```

### How a frame runs

The `Editor` owns the loop. Window events go to the toolkit's `Ui` first; what
the interface does not take, because the pointer is over the open middle of the
dock and nothing is captured, becomes a `ViewportInput` for
`Viewport::update`. The app then drains the game process's output into the
console, syncs the preview with the current document, builds the interface from
`EditorState`, and asks the `Renderer` to draw the preview's 3D frame into the
rectangle the dock leaves open (`Renderer::render_in`) with the interface and
the viewport overlay over it. `Message` is the one enum of the app; each panel
nests its own message type. Panels build elements from `&EditorState` and send
`fr_authoring` commands; a gesture such as dragging a value runs through
`begin_gesture`, `preview` and `commit_gesture`, so a whole drag is one undo
entry.

### Panels and the dock

- **Hierarchy**: the current document as the root row (with the unsaved marker)
  and its entities and prefab instances beneath, in order. Click selects (ctrl
  toggles, shift extends), double click or F2 renames, dragging a row onto
  another reparents, and the context menu creates (from the subsystems' create
  entries), duplicates and deletes.
- **Project**: the asset root's folders and files. Double-clicking a scene or a
  prefab opens it as a document, a model adds a Mesh Renderer entity to the
  current document as one undo entry, and the Create menu makes folders,
  scenes and prefabs with their `.meta` sidecars.
- **Inspector**: one section per component of the selected entity, with
  typed editors generated from the component schema: checkbox for booleans,
  drag values for numbers, a vec3 row, a colour field with a popup editor, a
  combo box for enums, a text field for strings and an asset slot. Add
  Component lists what the schema rules allow and says why the rest cannot be
  added; adding a component pulls its prerequisites in as the same undo entry.
  With nothing selected a scene shows its settings.
- **Viewport**: the live preview in the dock's open middle. Alt+left or middle
  drag orbits, shift+middle pans, right drag looks around with WASD and Q/E to
  fly, the wheel zooms and F frames the selection. Click selects by casting an
  `fr_math::Ray3d` against the entity bounds. Q, W, E and R pick the Select,
  Move, Rotate and Scale tools; the gizmo works along an axis, X switches local
  and world handles and Ctrl snaps.
- **Console**: the editor's messages, the preview's diagnostics and the game's
  output.

The panels sit in the toolkit's dock: a `DockTree` of splits and tab groups
around one open middle. The default is the hierarchy above the project on the
left, the inspector on the right and the console along the bottom. The tree is
serialised to `layout.txt` in the per-user config directory whenever it
changes; on start it is reconciled with the panels the program has, and an
unreadable file is reported in the console and replaced by the default.
View, Reset Layout restores the default.

## Nothing simulates while authoring

The preview realizes the document through the engine's adapters but never steps
the physics world and never plays a behaviour, so rigid bodies, colliders and
ragdolls are inert data: a dynamic body stays where it was placed. Game
components that have no registered schema are preserved in the document and
shown as unavailable.

## The toolkit boundary

The editor's interface is `fr_ui`: no window and no device, one dark theme and
one icon set. A screen is a function from the app's state to a tree of
elements, rebuilt every frame and generic over the message type.

| Lives in | What |
| --- | --- |
| `fr_ui` | Reusable controls with no knowledge of documents: layout, text, scroll areas, splits, popups and tooltips, buttons, checkboxes, text fields, drag values, sliders, combos, colour editors, tabs, trees, menus, the dock |
| `fr_editor` | Widgets that know the editor's data: property rows over component schemas, asset slots, the hierarchy and project trees, every panel |

A control two panels could use belongs in `fr_ui`; one that reads a schema, a
document or a stage belongs in `fr_editor`.

## Adding a subsystem

Adding support for a new engine subsystem means adding one folder under
`crates/fr_editor/src/subsystems/` and registering it in
`register_builtin_subsystems` in `subsystems/registry.rs`. It never means
editing `viewport/`, `preview/` or `fr_authoring`. A folder implements the
`Subsystem` trait for one component type key and contributes whatever it needs:

| Contribution | Responsibility |
| --- | --- |
| `icon` | The icon the hierarchy and inspector show for the component |
| `bounds` | Local-space bounds, which picking and framing use |
| `gizmo` | 3D line segments drawn into a `GizmoSink` for the selected entity |
| `create_entries` | Entries of the hierarchy's Create menu |
| `inspector` | Optional specialised `InspectorRow`s; a type without them gets rows generated from its schema |

The built-in components are contributed exactly like this: Transform,
MeshRenderer, Camera, Light, and RigidBody, Collider and Ragdoll in `physics/`.
Rows are plain data (`InspectorRow` with a `RowValue` and an apply closure
returning a `Command`), so a subsystem has no interface code.

```rust
pub fn register(registry: &mut SubsystemRegistry) {
    registry.add(Rc::new(LightSubsystem));
}
```

then add `light::register(registry);` to `register_builtin_subsystems`.

## Play

The editor has no play mode and loads no game code. **Play, Save and Play**
(F5) saves every dirty document first; a save failure cancels Play. It then
launches the project's `game_executable`, or, when the project names none,
`cargo run --release --manifest-path <project>/Cargo.toml`, as a separate
process in the asset root, passing `--project <root>` and `--scene <path>`
(`fr_authoring::launch_plan`). Its standard output and error are drained by
reader threads into the Console panel. **Stop** (Shift+F5) kills the process
group and the console reports `game exited with code N`. **Build and Play**
runs the project's cargo build steps first (`fr_project::build_steps`) and
launches only if they succeed. A crash in the game leaves the editor and its
unsaved work intact.

## Verification

```sh
cargo run -p fr_editor -- ~/Documents/forge/demo --capture build/editor.png --frames 6
WGPU_VALIDATION=1 timeout 8 target/release/forge_editor ~/Documents/forge/demo
```

Read the capture to check the hierarchy, project, inspector and viewport. The
headless parts (`fr_authoring`, the options and settings, the run planner) are
checked with throwaway programs in `~/.cache/scratch/forge`.
