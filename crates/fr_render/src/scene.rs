//! The description of one 3D frame: a camera, lights and mesh instances.

use fr_core::{Light, Mat4, MaterialId, MeshId, Transform, Vec3, look_at, perspective};

/// The default vertical field of view, in radians.
const DEFAULT_FOV_Y: f32 = std::f32::consts::FRAC_PI_3;

/// A perspective camera looking from a position at a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Where the camera is, in world space.
    pub position: Vec3,
    /// The point the camera looks at.
    pub target: Vec3,
    /// The direction that is up on screen.
    pub up: Vec3,
    /// The vertical field of view in radians.
    pub fov_y: f32,
    /// The distance to the near clip plane.
    pub near: f32,
    /// The distance to the far clip plane.
    pub far: f32,
    /// The multiplier applied to scene radiance before tonemapping.
    pub exposure: f32,
}

impl Camera {
    /// The matrix taking world space into the camera's view space.
    pub fn view(&self) -> Mat4 {
        look_at(self.position, self.target, self.up)
    }

    /// The matrix taking view space into clip space for a viewport of `aspect` width over height.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        perspective(self.fov_y, aspect.max(f32::EPSILON), self.near, self.far)
    }
}

impl Default for Camera {
    /// A camera five units back on Z, looking at the origin with a 60 degree field of view.
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: DEFAULT_FOV_Y,
            near: 0.05,
            far: 500.0,
            exposure: 1.0,
        }
    }
}

/// Light that reaches every surface from the sky and the ground: a procedural
/// hemisphere gradient, lit from above by the sky colour and from below by the
/// ground colour, also reflected by shiny surfaces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbientLight {
    /// The linear colour of the light from straight above.
    pub sky_color: Vec3,
    /// The linear colour of the light from straight below.
    pub ground_color: Vec3,
    /// How bright the light is.
    pub intensity: f32,
}

impl Default for AmbientLight {
    /// A faint blue sky over a dim warm ground.
    fn default() -> Self {
        Self {
            sky_color: Vec3::new(0.55, 0.7, 1.0),
            ground_color: Vec3::new(0.35, 0.3, 0.25),
            intensity: 0.3,
        }
    }
}

/// One mesh drawn with one material at one place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshInstance {
    /// The geometry to draw.
    pub mesh: MeshId,
    /// The material to draw it with.
    pub material: MaterialId,
    /// Where the mesh sits in the world.
    pub transform: Transform,
}

/// A mesh set uploaded from a model file: the instances and lights that make
/// it up, positioned relative to the model's own origin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    /// The meshes of the model with their materials and placements.
    pub instances: Vec<MeshInstance>,
    /// The lights the file defines.
    pub lights: Vec<Light>,
}

/// Everything the 3D pass draws in one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// The view the scene is seen from.
    pub camera: Camera,
    /// The lights: the first four directional ones and the first sixteen point
    /// and spot ones are used.
    pub lights: Vec<Light>,
    /// The sky and ground light added everywhere.
    pub ambient: AmbientLight,
    /// What is drawn; a scene with none draws no 3D at all.
    pub instances: Vec<MeshInstance>,
}

impl Scene {
    /// Removes the lights and instances, keeping the camera and ambient light.
    pub fn clear(&mut self) {
        self.lights.clear();
        self.instances.clear();
    }

    /// Adds one mesh with one material at `transform`.
    pub fn add(&mut self, mesh: MeshId, material: MaterialId, transform: Transform) {
        self.instances.push(MeshInstance {
            mesh,
            material,
            transform,
        });
    }

    /// Adds `light`, whichever kind it is.
    pub fn add_light(&mut self, light: impl Into<Light>) {
        self.lights.push(light.into());
    }

    /// Adds every instance and light of `model`, placed by `transform`.
    pub fn add_model(&mut self, model: &Model, transform: Transform) {
        self.instances
            .extend(model.instances.iter().map(|instance| MeshInstance {
                transform: instance.transform.then(&transform),
                ..*instance
            }));
        self.lights
            .extend(model.lights.iter().map(|light| light.placed(&transform)));
    }
}
