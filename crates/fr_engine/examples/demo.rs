//! The forge demo: a lit PBR scene with a sun and coloured point and spot lights that cast
//! shadows, an optional glTF model, and ragdolls dropped onto a pile of rigid bodies simulated
//! by `fr_physics` at a fixed rate.
//!
//! Pass the path of a `.gltf` or `.glb` file as the first argument to place it behind the pile.
//! Drag with the primary button to orbit and scroll to zoom. The panel toggles rotation,
//! shadows and V-sync, resets the scene, spawns bodies, pauses the simulation and reads out
//! the frame rate and the step, body and sleeping counts. Bodies turn dark when their island
//! falls asleep.

use std::collections::HashMap;
use std::path::PathBuf;

use fr_engine::assets::{
    AlphaMode, ImageData, MaterialData, SamplerData, capsule, cube, plane, sphere,
};
use fr_engine::physics::body::{BodyDef, BodyId, BodyType};
use fr_engine::physics::geometry::Geometry;
use fr_engine::physics::math::Pose;
use fr_engine::physics::ragdoll::{Ragdoll, RagdollDef};
use fr_engine::physics::shape::{Material, ShapeDef};
use fr_engine::physics::world::{World, WorldDef};
use fr_engine::ui::{Div, Rgba, Styled, Theme, button, h_flex, switch, text, v_flex};
use fr_engine::{
    AmbientLight, App, Assets, ButtonState, Camera, DirectionalLight, FixedStep, Frame, Input,
    LoadError, MaterialId, MeshId, Model, PointLight, PointerButton, Quat, Scene, Skeleton,
    SpotLight, Transform, Vec2, Vec3, Vec4,
};

/// The sub-steps of every fixed step.
const SUB_STEPS: usize = 4;

/// Radians of orbit per pixel dragged.
const ORBIT_SPEED: f32 = 0.005;

/// How far one logical pixel of scrolling changes the zoom, as a fraction of the distance.
const ZOOM_SPEED: f32 = 0.002;

/// The width of the control panel, in logical pixels.
const PANEL_WIDTH: f32 = 240.0;

/// The spheres in each of the two display rows.
const COLUMNS: usize = 5;

/// The distance between neighbouring spheres of a row.
const SPACING: f32 = 1.4;

/// Radians per second the display objects turn while rotation is on.
const SPIN_SPEED: f32 = 0.6;

/// The camera's starting yaw.
const START_YAW: f32 = 0.5;

/// The camera's starting pitch.
const START_PITCH: f32 = 0.5;

/// The camera's starting distance.
const START_DISTANCE: f32 = 6.5;

/// The point the camera looks at, above the middle of the pile.
const CAMERA_TARGET: Vec3 = Vec3::new(0.0, 1.0, 0.0);

/// The number of bodies the pile starts with.
const PILE_BODIES: usize = 9;

/// The largest number of bodies the demo allows before spawning stops.
const MAX_BODIES: usize = 400;

/// What the controls of the panel send.
#[derive(Clone, Copy, Debug)]
enum Message {
    /// Rebuilds the physics scene from scratch.
    Reset,
    /// Drops another ragdoll.
    SpawnRagdoll,
    /// Drops another box.
    SpawnBox,
    /// Stops or resumes the simulation.
    TogglePause,
    /// Turns the rotation of the display objects on or off.
    ToggleRotation,
    /// Turns shadows on or off for every light.
    ToggleShadows,
    /// Turns waiting for the display's refresh on or off.
    ToggleVsync,
    /// Puts the camera back where it started.
    ResetCamera,
}

/// The kind of body a visual stands for, which picks its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// A box of the pile.
    Box,
    /// A sphere of the pile.
    Ball,
    /// A capsule of the pile.
    Pill,
    /// A part of a ragdoll.
    Limb,
}

/// One body that is drawn.
#[derive(Clone, Copy, Debug)]
struct Visual {
    /// The body in the physics world.
    body: BodyId,
    /// What colour it is drawn in.
    class: Class,
}

/// The materials of one class: awake and asleep.
#[derive(Clone, Copy, Debug)]
struct ClassMaterials {
    /// The material while the body moves.
    awake: MaterialId,
    /// The material while the body sleeps.
    asleep: MaterialId,
}

/// The handles of everything the scene draws.
struct Scenery {
    /// The unit cube.
    cube: MeshId,
    /// The sphere of diameter one.
    ball: MeshId,
    /// The ground plane.
    ground: MeshId,
    /// The capsule meshes by radius and core length in millimetres.
    capsules: HashMap<(u32, u32), MeshId>,
    /// The ground's checkered material.
    ground_material: MaterialId,
    /// The material of the bone markers.
    marker_material: MaterialId,
    /// The materials of the first display row, metallic from none to full.
    metallic_row: Vec<MaterialId>,
    /// The materials of the second display row, roughness from polished to matte.
    roughness_row: Vec<MaterialId>,
    /// The translucent cube's material.
    glass_material: MaterialId,
    /// The glowing sphere's material.
    glow_material: MaterialId,
    /// The solid display cube's material.
    cube_material: MaterialId,
    /// The box materials.
    boxes: ClassMaterials,
    /// The sphere materials.
    balls: ClassMaterials,
    /// The capsule materials.
    pills: ClassMaterials,
    /// The ragdoll materials.
    limbs: ClassMaterials,
}

impl Scenery {
    /// The materials of `class`.
    fn materials(&self, class: Class) -> ClassMaterials {
        match class {
            Class::Box => self.boxes,
            Class::Ball => self.balls,
            Class::Pill => self.pills,
            Class::Limb => self.limbs,
        }
    }
}

/// The simulated world, the display objects and the camera that looks at them.
struct Demo {
    /// The physics world.
    world: World,
    /// The bodies that are drawn.
    visuals: Vec<Visual>,
    /// The ragdolls in the world.
    ragdolls: Vec<Ragdoll>,
    /// The skeleton every ragdoll is built from.
    skeleton: Skeleton,
    /// Whether the simulation is stopped.
    paused: bool,
    /// The state of the pseudo random number generator that places spawned bodies.
    random: u32,
    /// The number of ragdolls ever spawned, which gives each its collision group.
    spawned: i32,
    /// How far this frame is into the next fixed step.
    interpolation: f32,
    /// Whether the display objects are turning.
    rotating: bool,
    /// Whether the lights cast shadows.
    shadows: bool,
    /// Whether presenting waits for the display's refresh.
    vsync: bool,
    /// How far the display objects have turned, in radians.
    angle: f32,
    /// A smoothed frames-per-second reading.
    fps: f32,
    /// The path of the model given on the command line.
    model_path: Option<PathBuf>,
    /// The loaded model.
    model: Option<Model>,
    /// The orbit angle around the vertical axis, in radians.
    yaw: f32,
    /// The orbit angle above the ground, in radians.
    pitch: f32,
    /// The distance from the camera to its target.
    distance: f32,
    /// The pointer position while the orbit button is held.
    drag_from: Option<(f32, f32)>,
    /// The latest pointer position.
    pointer: Option<(f32, f32)>,
    /// The meshes and materials, once loaded.
    scenery: Option<Scenery>,
    /// A line shown at the bottom of the panel.
    status: String,
}

/// A grey checkerboard image of 8-pixel squares, as sRGB colour.
fn checkerboard() -> ImageData {
    let side = 64u32;
    let pixels = (0..side * side)
        .flat_map(|index| {
            let (x, y) = (index % side, index / side);
            let shade = if (x / 8 + y / 8) % 2 == 0 { 190 } else { 95 };
            [shade, shade, shade, 255]
        })
        .collect();
    ImageData {
        width: side,
        height: side,
        pixels,
    }
}

/// The key of a capsule mesh: the radius and the core length in millimetres.
fn capsule_key(radius: f32, length: f32) -> (u32, u32) {
    (
        (radius * 1000.0).round() as u32,
        (length * 1000.0).round() as u32,
    )
}

/// The awake and asleep materials of one colour.
fn class_materials(
    assets: &mut Assets,
    color: [f32; 3],
    roughness: f32,
) -> Result<ClassMaterials, LoadError> {
    let dim = color.map(|channel| channel * 0.55);
    Ok(ClassMaterials {
        awake: assets.add_material(&MaterialData::solid(color, 0.0, roughness))?,
        asleep: assets.add_material(&MaterialData::solid(dim, 0.0, roughness))?,
    })
}

/// The capsule keys of every capsule shape of a ragdoll and of the pile.
fn capsule_keys(skeleton: &Skeleton) -> Vec<(u32, u32)> {
    let mut world = World::new(WorldDef::default());
    let mut keys: Vec<(u32, u32)> = vec![capsule_key(0.3, 1.0)];
    if let Ok(ragdoll) = Ragdoll::build(&mut world, skeleton, &RagdollDef::default()) {
        for body in ragdoll.bodies() {
            for shape in world.body_shapes(body) {
                if let Some(Geometry::Capsule(capsule)) = world.shape_geometry(shape) {
                    let length = (capsule.center2() - capsule.center1()).length();
                    keys.push(capsule_key(capsule.radius, length));
                }
            }
        }
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Uploads the meshes and materials the scene is made of.
fn build_scenery(assets: &mut Assets, skeleton: &Skeleton) -> Result<Scenery, LoadError> {
    let checker = assets.add_texture(&checkerboard(), SamplerData::default(), true)?;
    let ground_material = assets.add_material(&MaterialData {
        base_color_texture: Some(checker),
        roughness: 0.9,
        ..MaterialData::default()
    })?;
    let mut capsules = HashMap::new();
    for (radius, length) in capsule_keys(skeleton) {
        let mesh = assets.add_mesh(&capsule(
            radius as f32 / 1000.0,
            length as f32 / 1000.0,
            24,
            8,
        ))?;
        capsules.insert((radius, length), mesh);
    }
    let step = |index: usize| index as f32 / (COLUMNS - 1) as f32;
    let metallic_row = (0..COLUMNS)
        .map(|index| {
            assets.add_material(&MaterialData::solid([0.95, 0.55, 0.3], step(index), 0.35))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let roughness_row = (0..COLUMNS)
        .map(|index| {
            let roughness = 0.05 + 0.95 * step(index);
            assets.add_material(&MaterialData::solid([0.9, 0.9, 0.92], 1.0, roughness))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Scenery {
        cube: assets.add_mesh(&cube(1.0))?,
        ball: assets.add_mesh(&sphere(0.5, 32, 16))?,
        ground: assets.add_mesh(&plane(Vec2::splat(80.0), 40.0))?,
        capsules,
        ground_material,
        marker_material: assets.add_material(&MaterialData::solid([0.05, 0.05, 0.06], 0.0, 0.4))?,
        metallic_row,
        roughness_row,
        glass_material: assets.add_material(&MaterialData {
            base_color: Vec4::new(0.1, 0.5, 0.8, 0.35),
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            roughness: 0.15,
            ..MaterialData::default()
        })?,
        glow_material: assets.add_material(&MaterialData {
            emissive: Vec3::new(1.0, 0.55, 0.2) * 2.5,
            roughness: 0.6,
            ..MaterialData::default()
        })?,
        cube_material: assets.add_material(&MaterialData::solid([0.2, 0.45, 0.9], 0.0, 0.4))?,
        boxes: class_materials(assets, [0.2, 0.45, 0.9], 0.45)?,
        balls: class_materials(assets, [0.95, 0.5, 0.2], 0.35)?,
        pills: class_materials(assets, [0.3, 0.8, 0.4], 0.4)?,
        limbs: class_materials(assets, [0.95, 0.3, 0.25], 0.6)?,
    })
}

impl Demo {
    /// A demo with the pile and one ragdoll, loading `model_path` when there is one.
    fn new(model_path: Option<PathBuf>) -> Self {
        let mut demo = Self {
            world: World::new(WorldDef::default()),
            visuals: Vec::new(),
            ragdolls: Vec::new(),
            skeleton: Skeleton::humanoid(),
            paused: false,
            random: 0x1234_5678,
            spawned: 0,
            interpolation: 0.0,
            rotating: true,
            shadows: true,
            vsync: false,
            angle: 0.0,
            fps: 0.0,
            model_path,
            model: None,
            yaw: START_YAW,
            pitch: START_PITCH,
            distance: START_DISTANCE,
            drag_from: None,
            pointer: None,
            scenery: None,
            status: String::new(),
        };
        demo.reset();
        demo
    }

    /// A pseudo random number in zero to one.
    fn next_random(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        (self.random >> 8) as f32 / 16_777_216.0
    }

    /// A pseudo random number in `low` to `high`.
    fn random_in(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.next_random()
    }

    /// Empties the world and builds the ground, the pile and one ragdoll.
    fn reset(&mut self) {
        self.world = World::new(WorldDef::default());
        self.visuals.clear();
        self.ragdolls.clear();
        self.spawned = 0;
        self.random = 0x1234_5678;
        let ground = self
            .world
            .create_body(&BodyDef::new(BodyType::Static).at(Vec3::new(0.0, -0.5, 0.0)));
        self.world.create_shape(
            ground,
            &ShapeDef::new(Geometry::cuboid(Vec3::new(40.0, 0.5, 40.0))),
        );
        for index in 0..PILE_BODIES {
            self.spawn_pile_body(index);
        }
        self.spawn_ragdoll();
        self.spawn_ragdoll();
    }

    /// Drops one box, ball or capsule of the pile.
    fn spawn_pile_body(&mut self, index: usize) {
        let x = self.random_in(-1.8, 1.8);
        let z = self.random_in(-1.8, 1.8);
        let y = 0.8 + index as f32 * 0.35;
        let turn = Quat::from_rotation_x(self.random_in(0.0, 3.0))
            * Quat::from_rotation_y(self.random_in(0.0, 3.0))
            * Quat::from_rotation_z(self.random_in(0.0, 3.0));
        let material = Material {
            friction: 0.6,
            rolling_resistance: 0.1,
            ..Material::default()
        };
        let (geometry, class) = match index % 3 {
            0 => (Geometry::cuboid(Vec3::new(0.35, 0.25, 0.3)), Class::Box),
            1 => (Geometry::sphere(0.3), Class::Ball),
            _ => (Geometry::capsule_y(0.5, 0.3), Class::Pill),
        };
        let body = self
            .world
            .create_body(&BodyDef::dynamic(Vec3::new(x, y, z)).rotated(turn));
        self.world
            .create_shape(body, &ShapeDef::new(geometry).with_material(material));
        self.visuals.push(Visual { body, class });
    }

    /// Drops one ragdoll above the pile.
    fn spawn_ragdoll(&mut self) {
        if self.world.stats().bodies >= MAX_BODIES {
            self.status = String::from("body limit reached");
            return;
        }
        self.spawned += 1;
        let position = Vec3::new(
            self.random_in(-1.5, 1.5),
            self.random_in(3.5, 5.0),
            self.random_in(-1.5, 1.5),
        );
        let rotation = Quat::from_rotation_y(self.random_in(0.0, 6.0))
            * Quat::from_rotation_x(self.random_in(-0.4, 0.4))
            * Quat::from_rotation_z(self.random_in(-0.4, 0.4));
        let def = RagdollDef {
            position,
            rotation,
            group: self.spawned,
            ..RagdollDef::default()
        };
        match Ragdoll::build(&mut self.world, &self.skeleton, &def) {
            Ok(ragdoll) => {
                for body in ragdoll.bodies() {
                    self.visuals.push(Visual {
                        body,
                        class: Class::Limb,
                    });
                }
                self.ragdolls.push(ragdoll);
            }
            Err(error) => self.status = format!("ragdoll failed: {error}"),
        }
    }

    /// Drops one box above the pile.
    fn spawn_box(&mut self) {
        if self.world.stats().bodies >= MAX_BODIES {
            self.status = String::from("body limit reached");
            return;
        }
        let position = Vec3::new(
            self.random_in(-1.5, 1.5),
            self.random_in(5.0, 7.0),
            self.random_in(-1.5, 1.5),
        );
        let body = self.world.create_body(&BodyDef::dynamic(position));
        self.world.create_shape(
            body,
            &ShapeDef::new(Geometry::cuboid(Vec3::new(0.4, 0.4, 0.4))),
        );
        self.visuals.push(Visual {
            body,
            class: Class::Box,
        });
    }

    /// The camera orbiting a point above the pile.
    fn camera(&self) -> Camera {
        let offset = Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        );
        Camera {
            position: CAMERA_TARGET + offset * self.distance,
            target: CAMERA_TARGET,
            ..Camera::default()
        }
    }

    /// Adds a row of display spheres along X at depth `z`, one per material.
    fn add_row(scene: &mut Scene, scenery: &Scenery, materials: &[MaterialId], z: f32) {
        let start = -(materials.len() as f32 - 1.0) * 0.5 * SPACING;
        for (index, &material) in materials.iter().enumerate() {
            let x = start + index as f32 * SPACING;
            scene.add(
                scenery.ball,
                material,
                Transform::from_translation(Vec3::new(x, 0.5, z)),
            );
        }
    }

    /// Adds the sun and the coloured point and spot lights.
    fn add_lights(&self, scene: &mut Scene) {
        let shadows = self.shadows;
        scene.add_light(DirectionalLight {
            direction: Vec3::new(0.6, -1.0, -0.4),
            intensity: 3.0,
            cast_shadows: shadows,
            ..DirectionalLight::default()
        });
        scene.add_light(PointLight {
            position: Vec3::new(-4.0, 2.0, 3.0),
            color: Vec3::new(1.0, 0.25, 0.2),
            intensity: 18.0,
            range: 12.0,
            cast_shadows: shadows,
        });
        scene.add_light(PointLight {
            position: Vec3::new(4.0, 2.2, -3.0),
            color: Vec3::new(0.2, 0.4, 1.0),
            intensity: 18.0,
            range: 12.0,
            cast_shadows: shadows,
        });
        scene.add_light(Self::spot_at(
            Vec3::new(0.0, 6.0, 5.0),
            Vec3::ZERO,
            Vec3::new(0.3, 1.0, 0.4),
            shadows,
        ));
        scene.add_light(Self::spot_at(
            Vec3::new(-6.0, 5.0, -4.0),
            Vec3::ZERO,
            Vec3::new(1.0, 0.8, 0.3),
            shadows,
        ));
    }

    /// A spot light at `position` aimed at `target`.
    fn spot_at(position: Vec3, target: Vec3, color: Vec3, cast_shadows: bool) -> SpotLight {
        SpotLight {
            position,
            direction: target - position,
            color,
            intensity: 90.0,
            range: 25.0,
            cast_shadows,
            ..SpotLight::default()
        }
    }

    /// Adds the meshes of one body to the scene.
    fn add_body(&self, scene: &mut Scene, scenery: &Scenery, visual: &Visual) {
        let Some(pose) = self
            .world
            .body_interpolated_pose(visual.body, self.interpolation)
        else {
            return;
        };
        let materials = scenery.materials(visual.class);
        let material = if self.world.is_body_awake(visual.body) {
            materials.awake
        } else {
            materials.asleep
        };
        for shape in self.world.body_shapes(visual.body) {
            let Some(geometry) = self.world.shape_geometry(shape) else {
                continue;
            };
            let (mesh, local, scale) = match geometry {
                Geometry::Sphere(ball) => (
                    scenery.ball,
                    Pose::from_position(ball.center),
                    Vec3::splat(ball.radius * 2.0),
                ),
                Geometry::Capsule(pill) => {
                    let axis = pill.center2() - pill.center1();
                    let length = axis.length();
                    let rotation = if length > 1e-6 {
                        Quat::from_rotation_arc(Vec3::Y, axis / length)
                    } else {
                        Quat::IDENTITY
                    };
                    let key = capsule_key(pill.radius, length);
                    let Some(&mesh) = scenery.capsules.get(&key) else {
                        continue;
                    };
                    (
                        mesh,
                        Pose::new((pill.center1() + pill.center2()) * 0.5, rotation),
                        Vec3::ONE,
                    )
                }
                Geometry::Hull(hull) => {
                    let bounds = hull.aabb();
                    (
                        scenery.cube,
                        Pose::from_position(bounds.center()),
                        bounds.extents() * 2.0,
                    )
                }
            };
            scene.add(
                mesh,
                material,
                pose.mul(&local).to_transform().with_scale(scale),
            );
        }
    }
}

impl App for Demo {
    type Message = Message;

    /// Uploads the primitives, the materials and the model named on the command line.
    fn init(&mut self, assets: &mut Assets) {
        match build_scenery(assets, &self.skeleton) {
            Ok(scenery) => self.scenery = Some(scenery),
            Err(error) => self.status = format!("scene failed: {error}"),
        }
        if let Some(path) = self.model_path.clone() {
            match assets.load_gltf(&path) {
                Ok(model) => self.model = Some(model),
                Err(error) => {
                    eprintln!("forge: {error}");
                    self.status = format!("model failed: {error}");
                }
            }
        }
    }

    /// Steps the physics world at the fixed rate.
    fn fixed_update(&mut self, step: &FixedStep) {
        if !self.paused {
            self.world.step(step.delta_seconds, SUB_STEPS);
        }
    }

    /// Remembers how far the frame is into the next step, turns the display objects and
    /// smooths the frame rate reading.
    fn update(&mut self, frame: &Frame) {
        self.interpolation = if self.paused {
            1.0
        } else {
            frame.interpolation
        };
        if self.rotating {
            self.angle += frame.delta_seconds * SPIN_SPEED;
        }
        let instant = 1.0 / frame.delta_seconds.max(1e-4);
        self.fps = if self.fps == 0.0 {
            instant
        } else {
            self.fps + (instant - self.fps) * 0.05
        };
    }

    /// A pale sky blue behind the scene.
    fn clear_color(&self, _theme: &Theme) -> Rgba {
        Rgba::new(0.45, 0.6, 0.8, 1.0)
    }

    /// Follows the V-sync switch.
    fn vsync(&self) -> bool {
        self.vsync
    }

    /// Applies a control of the panel.
    fn message(&mut self, message: Message) {
        match message {
            Message::Reset => self.reset(),
            Message::SpawnRagdoll => self.spawn_ragdoll(),
            Message::SpawnBox => self.spawn_box(),
            Message::TogglePause => self.paused = !self.paused,
            Message::ToggleRotation => self.rotating = !self.rotating,
            Message::ToggleShadows => self.shadows = !self.shadows,
            Message::ToggleVsync => self.vsync = !self.vsync,
            Message::ResetCamera => {
                self.yaw = START_YAW;
                self.pitch = START_PITCH;
                self.distance = START_DISTANCE;
            }
        }
    }

    /// Orbits on a primary-button drag and zooms on scroll.
    fn input(&mut self, input: &Input) {
        match input {
            Input::PointerMoved { x, y } => {
                if let Some((from_x, from_y)) = self.drag_from {
                    self.yaw -= (x - from_x) * ORBIT_SPEED;
                    self.pitch = (self.pitch + (y - from_y) * ORBIT_SPEED).clamp(0.05, 1.5);
                    self.drag_from = Some((*x, *y));
                }
                self.pointer = Some((*x, *y));
            }
            Input::PointerLeft => self.pointer = None,
            Input::PointerButton {
                button: PointerButton::Primary,
                state,
            } => {
                self.drag_from = match state {
                    ButtonState::Pressed => self.pointer,
                    ButtonState::Released => None,
                };
            }
            Input::Scrolled(delta) => {
                self.distance = (self.distance * (-delta.y * ZOOM_SPEED).exp()).clamp(3.0, 60.0);
            }
            Input::PointerButton { .. } | Input::Key(_) => {}
        }
    }

    /// Describes the camera, the lights, the display objects, the ground and every body.
    fn scene(&self, scene: &mut Scene) {
        scene.camera = self.camera();
        scene.ambient = AmbientLight {
            intensity: 0.2,
            ..AmbientLight::default()
        };
        self.add_lights(scene);
        let Some(scenery) = &self.scenery else {
            return;
        };
        scene.add(scenery.ground, scenery.ground_material, Transform::IDENTITY);
        Self::add_row(scene, scenery, &scenery.metallic_row, -5.0);
        Self::add_row(scene, scenery, &scenery.roughness_row, -6.8);
        let spin = |speed: f32| Quat::from_rotation_y(self.angle * speed);
        scene.add(
            scenery.cube,
            scenery.cube_material,
            Transform::from_translation(Vec3::new(-5.0, 0.5, 0.0)).with_rotation(spin(1.0)),
        );
        scene.add(
            scenery.cube,
            scenery.glass_material,
            Transform::from_translation(Vec3::new(5.0, 0.7, 0.0))
                .with_rotation(spin(-0.7))
                .with_scale(Vec3::splat(1.4)),
        );
        scene.add(
            scenery.ball,
            scenery.glow_material,
            Transform::from_translation(Vec3::new(0.0, 0.5, -3.5)),
        );
        if let Some(model) = &self.model {
            scene.add_model(
                model,
                Transform::from_translation(Vec3::new(0.0, 0.0, -10.0)).with_rotation(spin(0.5)),
            );
        }
        for visual in &self.visuals {
            self.add_body(scene, scenery, visual);
        }
        for ragdoll in &self.ragdolls {
            for bone in ragdoll.bone_interpolated_poses(&self.world, self.interpolation) {
                scene.add(
                    scenery.ball,
                    scenery.marker_material,
                    bone.with_scale(Vec3::splat(0.05)),
                );
            }
        }
    }

    /// The control panel in the top-left corner.
    fn view(&self, theme: &Theme) -> Div<Message> {
        let colors = &theme.colors;
        let stats = self.world.stats();
        let pause_label = if self.paused { "Resume" } else { "Pause" };
        let toggle = |on: bool, message: Message, label: &str| {
            h_flex()
                .gap(2)
                .items_center()
                .child(switch(on, message))
                .child(text(label.to_owned()).color(colors.text))
        };
        let readout = |line: String| text(line).text_sm().color(colors.text_muted);
        let panel = v_flex()
            .w_px(PANEL_WIDTH)
            .p(3)
            .gap(2)
            .bg(colors.surface)
            .rounded(theme.radius.lg)
            .border_1(colors.border_variant)
            .blocks_pointer()
            .child(
                text("Forge demo")
                    .text_lg()
                    .font_semibold()
                    .color(colors.text),
            )
            .child(toggle(
                self.rotating,
                Message::ToggleRotation,
                "Rotate objects",
            ))
            .child(toggle(self.shadows, Message::ToggleShadows, "Shadows"))
            .child(toggle(self.vsync, Message::ToggleVsync, "V-sync"))
            .child(button("Reset camera", Message::ResetCamera))
            .child(
                h_flex()
                    .gap(2)
                    .child(button("Reset", Message::Reset))
                    .child(button(pause_label, Message::TogglePause)),
            )
            .child(
                h_flex()
                    .gap(2)
                    .child(button("Ragdoll", Message::SpawnRagdoll))
                    .child(button("Box", Message::SpawnBox)),
            )
            .child(readout(format!(
                "{:.0} fps, step {}",
                self.fps, stats.steps
            )))
            .child(readout(format!(
                "{} bodies, {} joints",
                stats.bodies, stats.joints
            )))
            .child(readout(format!(
                "{} awake, {} sleeping",
                stats.awake_bodies, stats.sleeping_bodies
            )))
            .child(readout(format!(
                "{} contacts, {} islands",
                stats.touching_contacts, stats.islands
            )))
            .child(readout(String::from("Drag to orbit, scroll to zoom")))
            .child(readout(self.status.clone()));
        v_flex().w_full().h_full().p(2).items_start().child(panel)
    }
}

fn main() -> std::process::ExitCode {
    let demo = Demo::new(std::env::args_os().nth(1).map(PathBuf::from));
    match fr_engine::run("forge demo", demo) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("forge: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
