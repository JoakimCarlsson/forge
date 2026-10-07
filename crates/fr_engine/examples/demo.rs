//! The forge demo: a lit PBR scene with a sun and coloured point and spot lights that cast
//! shadows, an optional glTF model, and a ragdoll dropped on the ground and simulated by
//! `fr_physics` at a fixed rate.
//!
//! Pass the path of a `.gltf` or `.glb` file as the first argument to place it behind the ragdoll.
//!
//! Hold the right mouse button to look around and fly with W, A, S and D, E or space up, Q or
//! control down, shift for speed and the wheel to change the speed. Press the left mouse button
//! on the ragdoll to grab it, drag to pull it along a plane facing the camera and use the wheel
//! to move that plane towards or away from you. The panel resets the ragdoll, sets the
//! strength, damping and torque limit of its joint motors, pauses the simulation and toggles
//! V-sync.

mod controls {
    //! The movement keys that are held, as the direction they ask the camera to fly in.

    use fr_engine::input::Key;
    use fr_engine::math::Vec3;

    /// Which movement keys are down.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Controls {
        /// W: forward.
        forward: bool,
        /// S: back.
        back: bool,
        /// A: left.
        left: bool,
        /// D: right.
        right: bool,
        /// E or space: up.
        up: bool,
        /// Q or control: down.
        down: bool,
        /// Shift: faster.
        boost: bool,
    }

    impl Controls {
        /// Records `key` going down or, when `down` is false, coming up.
        pub fn set(&mut self, key: &Key, down: bool) {
            match key {
                Key::Character(text) => match text.to_lowercase().as_str() {
                    "w" => self.forward = down,
                    "s" => self.back = down,
                    "a" => self.left = down,
                    "d" => self.right = down,
                    "e" => self.up = down,
                    "q" => self.down = down,
                    _ => {}
                },
                Key::Space => self.up = down,
                Key::Control => self.down = down,
                Key::Shift => self.boost = down,
                _ => {}
            }
        }

        /// The wish along the right, up and forward axes, each from minus one to one.
        pub fn direction(&self) -> Vec3 {
            let axis = |positive: bool, negative: bool| f32::from(positive) - f32::from(negative);
            Vec3::new(
                axis(self.right, self.left),
                axis(self.up, self.down),
                axis(self.forward, self.back),
            )
        }

        /// Whether the boost key is down.
        pub fn boosting(&self) -> bool {
            self.boost
        }
    }
}
mod game {
    //! The game: the state of the world, the camera and the grab, and the application the engine
    //! drives.

    use std::path::PathBuf;

    use fr_engine::camera::{Camera, FlyController, Viewport};
    use fr_engine::input::{ButtonState, CursorMode, PointerButton};
    use fr_engine::light::{DirectionalLight, PointLight, SpotLight};
    use fr_engine::math::{Plane, Quat, Ray3d, Vec2, Vec3};
    use fr_engine::physics::body::{BodyDef, BodyId, BodyType};
    use fr_engine::physics::geometry::Geometry;
    use fr_engine::physics::joint::{JointId, JointMotor, MouseJoint};
    use fr_engine::physics::math::Pose;
    use fr_engine::physics::query::QueryFilter;
    use fr_engine::physics::rig::{
        HumanoidBones, Rig, RigDef, RigSettings, humanoid_rig, humanoid_t_pose,
    };
    use fr_engine::physics::shape::ShapeDef;
    use fr_engine::physics::world::{World, WorldDef};
    use fr_engine::render::{AmbientLight, Model, Scene};
    use fr_engine::transform::{Hierarchy, Transform};
    use fr_engine::ui::{Div, Rgba, Theme};
    use fr_engine::{App, Assets, FixedStep, Frame, Input};

    use crate::controls::Controls;
    use crate::message::Message;
    use crate::panel;
    use crate::scenery::{Scenery, build_scenery, capsule_key};

    /// The sub-steps of every fixed step.
    const SUB_STEPS: usize = 4;

    /// Where the camera starts.
    const CAMERA_START: Vec3 = Vec3::new(2.7, 4.1, 5.0);

    /// The point the camera starts out looking at.
    const CAMERA_TARGET: Vec3 = Vec3::new(0.0, 1.0, 0.0);

    /// Where the ragdoll is dropped from.
    const RAGDOLL_START: Vec3 = Vec3::new(0.0, 0.5, 0.0);

    /// The spring stiffness the ragdoll motors start with, in hertz.
    const MOTOR_STRENGTH: f32 = 1.0;

    /// The damping ratio the ragdoll motors start with.
    const MOTOR_DAMPING: f32 = 0.7;

    /// The torque limit the ragdoll motors start with, in newton metres.
    const MOTOR_TORQUE_LIMIT: f32 = 100.0;

    /// The stiffness of the grab spring, in hertz.
    const GRAB_HERTZ: f32 = 5.0;

    /// The damping ratio of the grab spring.
    const GRAB_DAMPING: f32 = 0.7;

    /// The strongest pull of the grab, in newtons per kilogram of the body held.
    const GRAB_FORCE_PER_MASS: f32 = 1000.0;

    /// The farthest a grab ray reaches, in metres.
    const GRAB_REACH: f32 = 100.0;

    /// How far one unit of scrolling moves a grabbed body along the view direction, in metres.
    const GRAB_DEPTH_PER_SCROLL: f32 = 0.005;

    /// The thickness of the line drawn from the grabbed point to its target, in metres.
    const GRAB_LINE_THICKNESS: f32 = 0.012;

    /// The diameter of the marker at the target of a grab, in metres.
    const GRAB_MARKER: f32 = 0.08;

    /// A body held by the pointer.
    #[derive(Clone, Copy, Debug)]
    struct Grab {
        /// The mouse joint that pulls the body.
        joint: JointId,
        /// The body that is held.
        body: BodyId,
        /// The grabbed point in the frame of the body.
        local_anchor: Vec3,
        /// The point the grab plane passes through when it has not been moved along the view.
        origin: Vec3,
        /// How far the grab plane has been moved along the view direction.
        depth: f32,
        /// Where the body is pulled to.
        target: Vec3,
    }

    /// What the panel shows and the state the controls change.
    pub struct Demo {
        /// The physics world.
        pub(crate) world: World,
        /// The static body the grab pulls against.
        ground: BodyId,
        /// The ragdoll in the world.
        ragdoll: Option<Rig>,
        /// The bones the ragdoll is built on.
        hierarchy: Hierarchy,
        /// What the ragdoll is made of.
        rig_def: RigDef,
        /// The motor every ragdoll joint is set to.
        pub(crate) motor: JointMotor,
        /// Whether the simulation is stopped.
        pub(crate) paused: bool,
        /// How far this frame is into the next fixed step.
        interpolation: f32,
        /// Whether presenting waits for the display's refresh.
        pub(crate) vsync: bool,
        /// A smoothed frames-per-second reading.
        pub(crate) fps: f32,
        /// The path of the model given on the command line.
        model_path: Option<PathBuf>,
        /// The loaded model.
        model: Option<Model>,
        /// The flying camera.
        fly: FlyController,
        /// The movement keys that are down.
        controls: Controls,
        /// Whether the right button is held, which looks around and flies.
        looking: bool,
        /// The body held by the pointer, if any.
        grab: Option<Grab>,
        /// The latest pointer position in logical pixels.
        pointer: Option<(f32, f32)>,
        /// The window in logical pixels, as of the last frame.
        viewport: Viewport,
        /// The meshes and materials, once loaded.
        scenery: Option<Scenery>,
        /// A line shown at the bottom of the panel.
        pub(crate) status: String,
    }

    impl Demo {
        /// A demo with the ragdoll dropped on the ground, loading `model_path` when there is one.
        ///
        /// # Errors
        ///
        /// Returns a description of what is wrong with the humanoid ragdoll.
        pub fn new(model_path: Option<PathBuf>) -> Result<Self, String> {
            let bones = HumanoidBones::default();
            let hierarchy = humanoid_t_pose(&bones).map_err(|error| error.to_string())?;
            let rig_def = humanoid_rig(&hierarchy, &bones).map_err(|error| error.to_string())?;
            let mut world = World::new(WorldDef::default());
            let ground = Self::create_ground(&mut world);
            let mut demo = Self {
                world,
                ground,
                ragdoll: None,
                hierarchy,
                rig_def,
                motor: JointMotor::new(MOTOR_STRENGTH, MOTOR_DAMPING, MOTOR_TORQUE_LIMIT),
                paused: false,
                interpolation: 0.0,
                vsync: false,
                fps: 0.0,
                model_path,
                model: None,
                fly: FlyController::looking_at(CAMERA_START, CAMERA_TARGET),
                controls: Controls::default(),
                looking: false,
                grab: None,
                pointer: None,
                viewport: Viewport::new(1.0, 1.0),
                scenery: None,
                status: String::new(),
            };
            demo.reset();
            Ok(demo)
        }

        /// Creates the ground in `world` and returns its body.
        fn create_ground(world: &mut World) -> BodyId {
            let ground =
                world.create_body(&BodyDef::new(BodyType::Static).at(Vec3::new(0.0, -0.5, 0.0)));
            world.create_shape(
                ground,
                &ShapeDef::new(Geometry::cuboid(Vec3::new(40.0, 0.5, 40.0))),
            );
            ground
        }

        /// Empties the world and drops one ragdoll on the ground.
        fn reset(&mut self) {
            self.world = World::new(WorldDef::default());
            self.ground = Self::create_ground(&mut self.world);
            self.grab = None;
            self.ragdoll = None;
            let settings = RigSettings {
                position: RAGDOLL_START,
                ..RigSettings::default()
            };
            match Rig::build(&mut self.world, &self.rig_def, &self.hierarchy, &settings) {
                Ok(mut ragdoll) => {
                    ragdoll.set_all_motors(&mut self.world, self.motor);
                    self.ragdoll = Some(ragdoll);
                }
                Err(error) => self.status = format!("ragdoll failed: {error}"),
            }
        }

        /// Sets the motor of every joint of the ragdoll to the panel's values.
        fn apply_motor(&mut self) {
            if let Some(ragdoll) = &mut self.ragdoll {
                ragdoll.set_all_motors(&mut self.world, self.motor);
            }
        }

        /// The camera the scene is drawn with and the pointer is cast through.
        fn camera(&self) -> Camera {
            let mut camera = Camera::default();
            self.fly.apply(&mut camera);
            camera
        }

        /// The ray from the camera through the pointer, when the pointer is in the window.
        fn pointer_ray(&self) -> Option<Ray3d> {
            let (x, y) = self.pointer?;
            self.camera()
                .viewport_to_world(self.viewport, Vec2::new(x, y))
        }

        /// Grabs the dynamic body under the pointer, if there is one.
        fn begin_grab(&mut self) {
            let Some(ray) = self.pointer_ray() else {
                return;
            };
            let Some(hit) = self
                .world
                .cast_ray_closest(&ray, GRAB_REACH, &QueryFilter::default())
            else {
                return;
            };
            if self.world.body_type(hit.body) != Some(BodyType::Dynamic) {
                return;
            }
            let (Some(pose), Some(mass)) = (
                self.world.body_pose(hit.body),
                self.world.body_mass(hit.body),
            ) else {
                return;
            };
            let joint = MouseJoint::new(GRAB_HERTZ, GRAB_DAMPING, GRAB_FORCE_PER_MASS * mass);
            if let Some(joint) =
                self.world
                    .create_mouse_joint(self.ground, hit.body, hit.point, joint)
            {
                self.grab = Some(Grab {
                    joint,
                    body: hit.body,
                    local_anchor: pose.inv_transform_point(hit.point),
                    origin: hit.point,
                    depth: 0.0,
                    target: hit.point,
                });
            }
        }

        /// Lets go of the body that is held, which keeps its velocity.
        fn end_grab(&mut self) {
            if let Some(grab) = self.grab.take() {
                self.world.destroy_joint(grab.joint);
            }
        }

        /// Moves the target of the grab to where the pointer meets the plane through the grabbed
        /// point that faces the camera.
        fn update_grab_target(&mut self) {
            let Some(grab) = self.grab else {
                return;
            };
            let Some(ray) = self.pointer_ray() else {
                return;
            };
            let forward = self.fly.forward();
            let plane = Plane::from_normal_and_point(forward, grab.origin + forward * grab.depth);
            if let (Some(distance), Some(held)) = (ray.intersect_plane(&plane), self.grab.as_mut())
            {
                held.target = ray.point_at(distance);
            }
        }

        /// Adds the sun and the coloured point and spot lights.
        fn add_lights(scene: &mut Scene) {
            scene.add_light(DirectionalLight {
                direction: Vec3::new(0.6, -1.0, -0.4),
                intensity: 3.0,
                cast_shadows: true,
                ..DirectionalLight::default()
            });
            scene.add_light(PointLight {
                position: Vec3::new(-4.0, 2.0, 3.0),
                color: Vec3::new(1.0, 0.25, 0.2),
                intensity: 18.0,
                range: 12.0,
                cast_shadows: true,
            });
            scene.add_light(PointLight {
                position: Vec3::new(4.0, 2.2, -3.0),
                color: Vec3::new(0.2, 0.4, 1.0),
                intensity: 18.0,
                range: 12.0,
                cast_shadows: true,
            });
            scene.add_light(Self::spot_at(
                Vec3::new(0.0, 6.0, 5.0),
                Vec3::ZERO,
                Vec3::new(0.3, 1.0, 0.4),
            ));
            scene.add_light(Self::spot_at(
                Vec3::new(-6.0, 5.0, -4.0),
                Vec3::ZERO,
                Vec3::new(1.0, 0.8, 0.3),
            ));
        }

        /// A shadow casting spot light at `position` aimed at `target`.
        fn spot_at(position: Vec3, target: Vec3, color: Vec3) -> SpotLight {
            SpotLight {
                position,
                direction: target - position,
                color,
                intensity: 90.0,
                range: 25.0,
                cast_shadows: true,
                ..SpotLight::default()
            }
        }

        /// Adds the meshes of one body to the scene.
        fn add_body(&self, scene: &mut Scene, scenery: &Scenery, body: BodyId) {
            let Some(pose) = self.world.body_interpolated_pose(body, self.interpolation) else {
                return;
            };
            let material = if self.world.is_body_awake(body) {
                scenery.limbs.awake
            } else {
                scenery.limbs.asleep
            };
            for shape in self.world.body_shapes(body) {
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

        /// Adds a thin line from the grabbed point to its target and a marker on the target.
        fn add_grab(&self, scene: &mut Scene, scenery: &Scenery) {
            let Some(grab) = &self.grab else {
                return;
            };
            let Some(pose) = self
                .world
                .body_interpolated_pose(grab.body, self.interpolation)
            else {
                return;
            };
            let anchor = pose.transform_point(grab.local_anchor);
            let span = grab.target - anchor;
            let length = span.length();
            if length > 1e-4 {
                scene.add(
                    scenery.cube,
                    scenery.marker_material,
                    Transform::from_translation((anchor + grab.target) * 0.5)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Z, span / length))
                        .with_scale(Vec3::new(GRAB_LINE_THICKNESS, GRAB_LINE_THICKNESS, length)),
                );
            }
            scene.add(
                scenery.ball,
                scenery.marker_material,
                Transform::from_translation(grab.target).with_scale(Vec3::splat(GRAB_MARKER)),
            );
        }
    }

    impl App for Demo {
        type Message = Message;

        /// Uploads the primitives, the materials and the model named on the command line.
        fn init(&mut self, assets: &mut Assets) {
            match build_scenery(assets, &self.hierarchy, &self.rig_def) {
                Ok(scenery) => self.scenery = Some(scenery),
                Err(error) => self.status = format!("scene failed: {error}"),
            }
            if let Some(path) = self.model_path.clone() {
                match assets.load_gltf(&path) {
                    Ok(model) => self.model = Some(model),
                    Err(error) => {
                        eprintln!("forge demo: {error}");
                        self.status = format!("model failed: {error}");
                    }
                }
            }
        }

        /// Steps the physics world at the fixed rate, pulling a grabbed body towards its target.
        fn fixed_update(&mut self, step: &FixedStep) {
            if self.paused {
                return;
            }
            if let Some(grab) = self.grab {
                self.world.set_mouse_joint_target(grab.joint, grab.target);
            }
            self.world.step(step.delta_seconds, SUB_STEPS);
        }

        /// Remembers how far the frame is into the next step, flies the camera, moves the grab
        /// target and smooths the frame rate reading.
        fn update(&mut self, frame: &Frame) {
            self.interpolation = if self.paused {
                1.0
            } else {
                frame.interpolation
            };
            self.viewport = Viewport::new(
                frame.width as f32 / frame.scale_factor,
                frame.height as f32 / frame.scale_factor,
            );
            if self.looking {
                self.fly.fly(
                    self.controls.direction(),
                    self.controls.boosting(),
                    frame.delta_seconds,
                );
            }
            self.update_grab_target();
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

        /// Hides and captures the cursor while the right button flies the camera.
        fn cursor_mode(&self) -> CursorMode {
            if self.looking {
                CursorMode::Captured
            } else {
                CursorMode::Normal
            }
        }

        /// Applies a control of the panel.
        fn message(&mut self, message: Message) {
            match message {
                Message::Reset => self.reset(),
                Message::TogglePause => self.paused = !self.paused,
                Message::ToggleVsync => self.vsync = !self.vsync,
                Message::SetMotorStrength(strength) => {
                    self.motor.strength = strength;
                    self.apply_motor();
                }
                Message::SetMotorDamping(damping) => {
                    self.motor.damping = damping;
                    self.apply_motor();
                }
                Message::SetMotorTorqueLimit(max_torque) => {
                    self.motor.max_torque = max_torque;
                    self.apply_motor();
                }
            }
        }

        /// Looks and flies while the right button is held, grabs and drags with the left button and
        /// reads the movement keys.
        fn input(&mut self, input: &Input) {
            match input {
                Input::PointerMoved { x, y } => self.pointer = Some((*x, *y)),
                Input::PointerLeft => self.pointer = None,
                Input::PointerMotion { dx, dy } => {
                    if self.looking {
                        self.fly.look(*dx, *dy);
                    }
                }
                Input::PointerButton {
                    button: PointerButton::Secondary,
                    state,
                } => match state {
                    ButtonState::Pressed => self.looking = self.grab.is_none(),
                    ButtonState::Released => self.looking = false,
                },
                Input::PointerButton {
                    button: PointerButton::Primary,
                    state,
                } => match state {
                    ButtonState::Pressed if !self.looking => self.begin_grab(),
                    ButtonState::Pressed => {}
                    ButtonState::Released => self.end_grab(),
                },
                Input::Scrolled(delta) => {
                    if self.looking {
                        self.fly.scroll(delta.y);
                    } else if let Some(grab) = &mut self.grab {
                        grab.depth += delta.y * GRAB_DEPTH_PER_SCROLL;
                    }
                }
                Input::Key(event) => self
                    .controls
                    .set(&event.key, event.state == ButtonState::Pressed),
                Input::PointerButton { .. } => {}
            }
        }

        /// Describes the camera, the lights, the ground, the model, the ragdoll and the grab.
        fn scene(&self, scene: &mut Scene) {
            scene.camera = self.camera();
            scene.ambient = AmbientLight {
                intensity: 0.2,
                ..AmbientLight::default()
            };
            Self::add_lights(scene);
            let Some(scenery) = &self.scenery else {
                return;
            };
            scene.add(scenery.ground, scenery.ground_material, Transform::IDENTITY);
            if let Some(model) = &self.model {
                scene.add_model(
                    model,
                    Transform::from_translation(Vec3::new(0.0, 0.0, -10.0)),
                );
            }
            if let Some(ragdoll) = &self.ragdoll {
                for body in ragdoll.bodies() {
                    self.add_body(scene, scenery, body);
                }
                for bone in ragdoll.bone_interpolated_poses(&self.world, self.interpolation) {
                    scene.add(
                        scenery.ball,
                        scenery.marker_material,
                        bone.with_scale(Vec3::splat(0.05)),
                    );
                }
            }
            self.add_grab(scene, scenery);
        }

        /// The control panel in the top-left corner.
        fn view(&self, theme: &Theme) -> Div<Message> {
            panel::panel(self, theme)
        }
    }
}
mod message {
    //! The messages the control panel sends.

    /// What the controls of the panel send.
    #[derive(Clone, Copy, Debug)]
    pub enum Message {
        /// Drops the ragdoll again.
        Reset,
        /// Stops or resumes the simulation.
        TogglePause,
        /// Turns waiting for the display's refresh on or off.
        ToggleVsync,
        /// Sets the stiffness of every ragdoll joint motor, in hertz.
        SetMotorStrength(f32),
        /// Sets the damping ratio of every ragdoll joint motor.
        SetMotorDamping(f32),
        /// Sets the torque limit of every ragdoll joint motor, in newton metres.
        SetMotorTorqueLimit(f32),
    }
}
mod panel {
    //! The control panel.

    use fr_engine::ui::{Div, Slider, Styled, Theme, button, h_flex, slider, switch, text, v_flex};

    use crate::game::Demo;
    use crate::message::Message;

    /// The width of the control panel, in logical pixels.
    const PANEL_WIDTH: f32 = 240.0;

    /// The highest spring stiffness the panel offers, in hertz.
    const MOTOR_STRENGTH_MAX: f32 = 20.0;

    /// The highest damping ratio the panel offers.
    const MOTOR_DAMPING_MAX: f32 = 2.0;

    /// The highest torque limit the panel offers, in newton metres.
    const MOTOR_TORQUE_LIMIT_MAX: f32 = 200.0;

    /// A caption over a slider.
    fn labelled_slider(theme: &Theme, caption: String, control: Slider<Message>) -> Div<Message> {
        v_flex()
            .w_full()
            .gap(1)
            .child(text(caption).text_sm().color(theme.colors.text))
            .child(control)
    }

    /// The control panel in the top-left corner: the ragdoll's reset and motor sliders, pause and
    /// V-sync, the readouts of the frame rate and the physics world, and the controls.
    pub fn panel(demo: &Demo, theme: &Theme) -> Div<Message> {
        let colors = &theme.colors;
        let stats = demo.world.stats();
        let pause_label = if demo.paused { "Resume" } else { "Pause" };
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
            .child(
                h_flex()
                    .gap(2)
                    .child(button("Reset ragdoll", Message::Reset))
                    .child(button(pause_label, Message::TogglePause)),
            )
            .child(labelled_slider(
                theme,
                format!("Motor strength {:.1} Hz", demo.motor.strength),
                slider(
                    demo.motor.strength,
                    0.0,
                    MOTOR_STRENGTH_MAX,
                    Message::SetMotorStrength,
                ),
            ))
            .child(labelled_slider(
                theme,
                format!("Motor damping {:.2}", demo.motor.damping),
                slider(
                    demo.motor.damping,
                    0.0,
                    MOTOR_DAMPING_MAX,
                    Message::SetMotorDamping,
                ),
            ))
            .child(labelled_slider(
                theme,
                format!("Torque limit {:.0} N m", demo.motor.max_torque),
                slider(
                    demo.motor.max_torque,
                    0.0,
                    MOTOR_TORQUE_LIMIT_MAX,
                    Message::SetMotorTorqueLimit,
                ),
            ))
            .child(
                h_flex()
                    .gap(2)
                    .items_center()
                    .child(switch(demo.vsync, Message::ToggleVsync))
                    .child(text("V-sync").color(colors.text)),
            )
            .child(readout(format!(
                "{:.0} fps, step {}",
                demo.fps, stats.steps
            )))
            .child(readout(format!(
                "{} bodies, {} joints",
                stats.bodies, stats.joints
            )))
            .child(readout(format!(
                "{} awake, {} sleeping",
                stats.awake_bodies, stats.sleeping_bodies
            )))
            .child(readout(String::from("Right mouse: look, WASD fly")))
            .child(readout(String::from("Q/E down/up, Shift fast")))
            .child(readout(String::from("Wheel: fly speed")))
            .child(readout(String::from("Left mouse: grab, wheel depth")))
            .child(readout(demo.status.clone()));
        v_flex().w_full().h_full().p(2).items_start().child(panel)
    }
}
mod scenery {
    //! The meshes and materials the scene is drawn with.

    use std::collections::HashMap;

    use fr_engine::image::{ImageData, SamplerData};
    use fr_engine::material::{MaterialData, MaterialId};
    use fr_engine::math::Vec2;
    use fr_engine::mesh::{MeshId, capsule, cube, plane, sphere};
    use fr_engine::physics::geometry::Geometry;
    use fr_engine::physics::rig::{Rig, RigDef, RigSettings};
    use fr_engine::physics::world::{World, WorldDef};
    use fr_engine::transform::Hierarchy;
    use fr_engine::{Assets, LoadError};

    /// The materials of the ragdoll: awake and asleep.
    #[derive(Clone, Copy, Debug)]
    pub struct LimbMaterials {
        /// The material while the body moves.
        pub awake: MaterialId,
        /// The material while the body sleeps.
        pub asleep: MaterialId,
    }

    /// The handles of everything the scene draws.
    pub struct Scenery {
        /// The unit cube.
        pub cube: MeshId,
        /// The sphere of diameter one.
        pub ball: MeshId,
        /// The ground plane.
        pub ground: MeshId,
        /// The capsule meshes by radius and core length in millimetres.
        pub capsules: HashMap<(u32, u32), MeshId>,
        /// The ground's checkered material.
        pub ground_material: MaterialId,
        /// The material of the bone markers and the grab line.
        pub marker_material: MaterialId,
        /// The ragdoll materials.
        pub limbs: LimbMaterials,
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
    pub fn capsule_key(radius: f32, length: f32) -> (u32, u32) {
        (
            (radius * 1000.0).round() as u32,
            (length * 1000.0).round() as u32,
        )
    }

    /// The awake and asleep materials of one colour.
    fn limb_materials(
        assets: &mut Assets,
        color: [f32; 3],
        roughness: f32,
    ) -> Result<LimbMaterials, LoadError> {
        let dim = color.map(|channel| channel * 0.55);
        Ok(LimbMaterials {
            awake: assets.add_material(&MaterialData::solid(color, 0.0, roughness))?,
            asleep: assets.add_material(&MaterialData::solid(dim, 0.0, roughness))?,
        })
    }

    /// The capsule keys of every capsule shape of a ragdoll.
    fn capsule_keys(hierarchy: &Hierarchy, rig_def: &RigDef) -> Vec<(u32, u32)> {
        let mut world = World::new(WorldDef::default());
        let mut keys: Vec<(u32, u32)> = Vec::new();
        if let Ok(ragdoll) = Rig::build(&mut world, rig_def, hierarchy, &RigSettings::default()) {
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
    pub fn build_scenery(
        assets: &mut Assets,
        hierarchy: &Hierarchy,
        rig_def: &RigDef,
    ) -> Result<Scenery, LoadError> {
        let checker = assets.add_texture(&checkerboard(), SamplerData::default(), true)?;
        let ground_material = assets.add_material(&MaterialData {
            base_color_texture: Some(checker),
            roughness: 0.9,
            ..MaterialData::default()
        })?;
        let mut capsules = HashMap::new();
        for (radius, length) in capsule_keys(hierarchy, rig_def) {
            let mesh = assets.add_mesh(&capsule(
                radius as f32 / 1000.0,
                length as f32 / 1000.0,
                24,
                8,
            ))?;
            capsules.insert((radius, length), mesh);
        }
        Ok(Scenery {
            cube: assets.add_mesh(&cube(1.0))?,
            ball: assets.add_mesh(&sphere(0.5, 32, 16))?,
            ground: assets.add_mesh(&plane(Vec2::splat(80.0), 40.0))?,
            capsules,
            ground_material,
            marker_material: assets.add_material(&MaterialData::solid(
                [0.05, 0.05, 0.06],
                0.0,
                0.4,
            ))?,
            limbs: limb_materials(assets, [0.95, 0.3, 0.25], 0.6)?,
        })
    }
}

use std::path::PathBuf;
use std::process::ExitCode;

use crate::game::Demo;

/// The title of the game's window.
const WINDOW_TITLE: &str = "forge demo";

/// Opens the demo window, with the model named by the first argument when there is one.
fn main() -> ExitCode {
    let demo = match Demo::new(std::env::args_os().nth(1).map(PathBuf::from)) {
        Ok(demo) => demo,
        Err(error) => {
            eprintln!("forge demo: {error}");
            return ExitCode::FAILURE;
        }
    };
    match fr_engine::run(WINDOW_TITLE, demo) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("forge demo: {error}");
            ExitCode::FAILURE
        }
    }
}
