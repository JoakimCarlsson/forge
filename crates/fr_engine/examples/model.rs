//! Orbits a camera around lit primitives, a tiled ground and an optional glTF
//! model under a sun and coloured point and spot lights that cast shadows, with
//! a small UI overlay.
//!
//! Pass the path of a `.gltf` or `.glb` file as the first argument to place it
//! behind the primitives; without one, only the primitives are shown. Drag with
//! the primary button to orbit and scroll to zoom.

use std::path::PathBuf;

use fr_engine::assets::{AlphaMode, ImageData, MaterialData, SamplerData, cube, plane, sphere};
use fr_engine::ui::{Div, Rgba, Styled, Theme, button, h_flex, switch, text, v_flex};
use fr_engine::{
    AmbientLight, App, Assets, ButtonState, Camera, DirectionalLight, Frame, Input, LoadError,
    MaterialId, MeshId, Model, PointLight, PointerButton, Quat, Scene, SpotLight, Transform, Vec2,
    Vec3,
};

/// The spheres in each of the two material rows.
const COLUMNS: usize = 5;

/// The distance between neighbouring spheres.
const SPACING: f32 = 1.4;

/// The radius of each sphere.
const RADIUS: f32 = 0.5;

/// Radians of orbit per pixel dragged.
const ORBIT_SPEED: f32 = 0.005;

/// How far one logical pixel of scrolling changes the zoom, as a fraction of the distance.
const ZOOM_SPEED: f32 = 0.002;

/// Radians per second the objects turn while rotation is on.
const SPIN_SPEED: f32 = 0.6;

/// The width of the control panel, in logical pixels.
const PANEL_WIDTH: f32 = 240.0;

/// What the controls of the overlay send.
#[derive(Clone, Copy, Debug)]
enum Message {
    /// Turns object rotation on or off.
    ToggleRotation,
    /// Turns shadows on or off for every light.
    ToggleShadows,
    /// Turns waiting for the display's refresh on or off.
    ToggleVsync,
    /// Puts the camera back where it started.
    ResetCamera,
}

/// The handles of everything the scene draws.
struct Scenery {
    /// The sphere shared by both rows.
    sphere: MeshId,
    /// The cube.
    cube: MeshId,
    /// The ground plane.
    ground: MeshId,
    /// The ground's checkered material.
    ground_material: MaterialId,
    /// The materials of the first row, metallic from none to full.
    metallic_row: Vec<MaterialId>,
    /// The materials of the second row, roughness from polished to matte.
    roughness_row: Vec<MaterialId>,
    /// The solid cube's material.
    cube_material: MaterialId,
    /// The translucent cube's material.
    glass_material: MaterialId,
    /// The glowing sphere's material.
    glow_material: MaterialId,
}

/// The state the window and the scene are drawn from.
struct Viewer {
    /// The orbit angle around the vertical axis, in radians.
    yaw: f32,
    /// The orbit angle above the ground, in radians.
    pitch: f32,
    /// The distance from the camera to its target.
    distance: f32,
    /// Whether the objects are turning.
    rotating: bool,
    /// Whether the lights cast shadows.
    shadows: bool,
    /// Whether presenting waits for the display's refresh.
    vsync: bool,
    /// How far the objects have turned, in radians.
    angle: f32,
    /// The pointer position while the orbit button is held.
    drag_from: Option<(f32, f32)>,
    /// The latest pointer position.
    pointer: Option<(f32, f32)>,
    /// The path of the model given on the command line.
    model_path: Option<PathBuf>,
    /// The loaded model.
    model: Option<Model>,
    /// The primitives, once loaded.
    scenery: Option<Scenery>,
    /// The line shown at the bottom of the panel.
    status: String,
    /// A smoothed frames-per-second reading.
    fps: f32,
}

/// The camera's starting yaw.
const START_YAW: f32 = 0.5;

/// The camera's starting pitch.
const START_PITCH: f32 = 0.35;

/// The camera's starting distance.
const START_DISTANCE: f32 = 9.0;

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

/// Uploads the primitives and materials the scene is made of.
fn build_scenery(assets: &mut Assets) -> Result<Scenery, LoadError> {
    let checker = assets.add_texture(&checkerboard(), SamplerData::default(), true)?;
    let ground_material = assets.add_material(&MaterialData {
        base_color_texture: Some(checker),
        roughness: 0.9,
        ..MaterialData::default()
    })?;

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
        sphere: assets.add_mesh(&sphere(RADIUS, 48, 24))?,
        cube: assets.add_mesh(&cube(1.0))?,
        ground: assets.add_mesh(&plane(Vec2::splat(60.0), 30.0))?,
        ground_material,
        metallic_row,
        roughness_row,
        cube_material: assets.add_material(&MaterialData::solid([0.2, 0.45, 0.9], 0.0, 0.4))?,
        glass_material: assets.add_material(&MaterialData {
            base_color: fr_engine::Vec4::new(0.1, 0.5, 0.8, 0.35),
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
    })
}

impl Viewer {
    /// A viewer with the camera at its starting place and rotation on.
    fn new(model_path: Option<PathBuf>) -> Self {
        Self {
            yaw: START_YAW,
            pitch: START_PITCH,
            distance: START_DISTANCE,
            rotating: true,
            shadows: true,
            vsync: false,
            angle: 0.0,
            drag_from: None,
            pointer: None,
            model_path,
            model: None,
            scenery: None,
            status: String::new(),
            fps: 0.0,
        }
    }

    /// The camera orbiting a point just above the ground.
    fn camera(&self) -> Camera {
        let target = Vec3::new(0.0, 0.3, 0.0);
        let offset = Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        );
        Camera {
            position: target + offset * self.distance,
            target,
            ..Camera::default()
        }
    }

    /// Adds a row of spheres along X at depth `z`, one per material.
    fn add_row(scene: &mut Scene, mesh: MeshId, materials: &[MaterialId], z: f32) {
        let start = -(materials.len() as f32 - 1.0) * 0.5 * SPACING;
        for (index, &material) in materials.iter().enumerate() {
            let x = start + index as f32 * SPACING;
            scene.add(
                mesh,
                material,
                Transform::from_translation(Vec3::new(x, 0.0, z)),
            );
        }
    }

    /// Adds the sun and the coloured point and spot lights.
    fn add_lights(&self, scene: &mut Scene) {
        let shadows = self.shadows;
        scene.add_light(DirectionalLight {
            direction: Vec3::new(0.7, -1.0, -0.5),
            intensity: 3.0,
            cast_shadows: shadows,
            ..DirectionalLight::default()
        });
        scene.add_light(PointLight {
            position: Vec3::new(-3.2, 1.6, 2.8),
            color: Vec3::new(1.0, 0.25, 0.2),
            intensity: 18.0,
            range: 12.0,
            cast_shadows: shadows,
        });
        scene.add_light(PointLight {
            position: Vec3::new(3.4, 1.8, -2.6),
            color: Vec3::new(0.2, 0.4, 1.0),
            intensity: 18.0,
            range: 12.0,
            cast_shadows: shadows,
        });
        scene.add_light(Self::spot_at(
            Vec3::new(0.0, 5.0, 4.5),
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(0.3, 1.0, 0.4),
            shadows,
        ));
        scene.add_light(Self::spot_at(
            Vec3::new(-5.5, 4.0, -3.5),
            Vec3::new(-1.0, -0.5, 0.0),
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

    /// Turns `angle` into a rotation about the vertical axis.
    fn spin(&self, speed: f32) -> Quat {
        Quat::from_rotation_y(self.angle * speed)
    }
}

impl App for Viewer {
    type Message = Message;

    /// Uploads the primitives and the model named on the command line.
    fn init(&mut self, assets: &mut Assets) {
        match build_scenery(assets) {
            Ok(scenery) => self.scenery = Some(scenery),
            Err(error) => self.status = format!("scene failed: {error}"),
        }
        let Some(path) = self.model_path.clone() else {
            self.status = String::from("no model given");
            return;
        };
        match assets.load_gltf(&path) {
            Ok(model) => {
                self.status = format!("{} meshes loaded", model.instances.len());
                self.model = Some(model);
            }
            Err(error) => {
                eprintln!("forge: {error}");
                self.status = format!("model failed: {error}");
            }
        }
    }

    /// Turns the objects and smooths the frame rate reading.
    fn update(&mut self, frame: &Frame) {
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

    /// Toggles rotation, shadows or V-sync, or resets the camera.
    fn message(&mut self, message: Message) {
        match message {
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
                self.distance = (self.distance * (-delta.y * ZOOM_SPEED).exp()).clamp(2.0, 40.0);
            }
            Input::PointerButton { .. } | Input::Key(_) => {}
        }
    }

    /// Describes the camera, the lights and every object.
    fn scene(&self, scene: &mut Scene) {
        scene.camera = self.camera();
        scene.ambient = AmbientLight {
            intensity: 0.12,
            ..AmbientLight::default()
        };
        self.add_lights(scene);
        let Some(scenery) = &self.scenery else {
            return;
        };
        scene.add(
            scenery.ground,
            scenery.ground_material,
            Transform::from_translation(Vec3::new(0.0, -RADIUS, 0.0)),
        );
        Self::add_row(scene, scenery.sphere, &scenery.metallic_row, -1.3);
        Self::add_row(scene, scenery.sphere, &scenery.roughness_row, 1.3);
        scene.add(
            scenery.cube,
            scenery.cube_material,
            Transform::from_translation(Vec3::new(-4.6, 0.0, 0.0)).with_rotation(self.spin(1.0)),
        );
        scene.add(
            scenery.cube,
            scenery.cube_material,
            Transform::from_translation(Vec3::new(-2.4, 1.0, 4.0))
                .with_scale(Vec3::new(0.8, 3.0, 0.8)),
        );
        scene.add(
            scenery.cube,
            scenery.glass_material,
            Transform::from_translation(Vec3::new(4.6, 0.0, 0.0))
                .with_rotation(self.spin(-0.7))
                .with_scale(Vec3::splat(1.4)),
        );
        scene.add(
            scenery.sphere,
            scenery.glow_material,
            Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
        );
        if let Some(model) = &self.model {
            scene.add_model(
                model,
                Transform::from_translation(Vec3::new(0.0, -RADIUS, -5.0))
                    .with_rotation(self.spin(0.5)),
            );
        }
    }

    /// The control panel in the top-left corner.
    fn view(&self, theme: &Theme) -> Div<Message> {
        let colors = &theme.colors;
        let panel = v_flex()
            .w_px(PANEL_WIDTH)
            .p(3)
            .gap(2)
            .bg(colors.surface)
            .rounded(theme.radius.lg)
            .border_1(colors.border_variant)
            .blocks_pointer()
            .child(text("Viewer").text_lg().font_semibold().color(colors.text))
            .child(
                h_flex()
                    .gap(2)
                    .items_center()
                    .child(switch(self.rotating, Message::ToggleRotation))
                    .child(text("Rotate objects").color(colors.text)),
            )
            .child(
                h_flex()
                    .gap(2)
                    .items_center()
                    .child(switch(self.shadows, Message::ToggleShadows))
                    .child(text("Shadows").color(colors.text)),
            )
            .child(
                h_flex()
                    .gap(2)
                    .items_center()
                    .child(switch(self.vsync, Message::ToggleVsync))
                    .child(text("V-sync").color(colors.text)),
            )
            .child(button("Reset camera", Message::ResetCamera))
            .child(
                text("Drag to orbit, scroll to zoom")
                    .text_sm()
                    .color(colors.text_muted),
            )
            .child(
                text(format!("{:.0} fps", self.fps))
                    .text_sm()
                    .color(colors.text_muted),
            )
            .child(text(self.status.clone()).text_sm().color(colors.text_muted));
        v_flex().w_full().h_full().p(2).items_start().child(panel)
    }
}

fn main() -> std::process::ExitCode {
    let viewer = Viewer::new(std::env::args_os().nth(1).map(PathBuf::from));
    match fr_engine::run("forge model", viewer) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("forge: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
