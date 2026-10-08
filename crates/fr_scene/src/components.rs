//! Components that describe authored objects without a graphics device.

use fr_app::ecs as bevy_ecs;
use fr_app::ecs::{component::Component, reflect::ReflectComponent};
use fr_app::reflect as bevy_reflect;
use fr_app::reflect::Reflect;
use fr_math::{Mat4, Quat, Vec3};
use fr_transform::Transform;

/// A persistent object identifier, independent of runtime entity allocation.
#[derive(Component, Reflect, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[reflect(Component)]
pub struct SceneId(pub String);

impl SceneId {
    /// Creates a new persistent identifier.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for SceneId {
    /// Creates a new persistent identifier.
    fn default() -> Self {
        Self::new()
    }
}

/// The name displayed for an object.
#[derive(Component, Reflect, Clone, Debug, Default, PartialEq, Eq)]
#[reflect(Component)]
pub struct Name(pub String);

/// An object's parent, addressed by its persistent identifier.
#[derive(Component, Reflect, Clone, Debug, PartialEq, Eq)]
#[reflect(Component)]
pub struct Parent(pub SceneId);

/// An asset source path, independent of an uploaded GPU handle.
#[derive(Reflect, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPath(pub String);

/// A model source to instantiate at an object's transform.
#[derive(Component, Reflect, Clone, Debug, Default, PartialEq, Eq)]
#[reflect(Component)]
pub struct ModelRef(pub AssetPath);

/// An editable transform relative to the parent, with quaternion rotation.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct LocalTransform {
    /// Position relative to the parent.
    pub translation: [f32; 3],
    /// Quaternion rotation in x, y, z, w order.
    pub rotation: [f32; 4],
    /// Scale along each local axis.
    pub scale: [f32; 3],
}

impl LocalTransform {
    /// Converts an engine transform into editable component data.
    pub fn new(transform: Transform) -> Self {
        Self {
            translation: transform.translation.to_array(),
            rotation: transform.rotation.to_array(),
            scale: transform.scale.to_array(),
        }
    }

    /// Converts the editable values into an engine transform.
    pub fn transform(&self) -> Transform {
        Transform {
            translation: Vec3::from_array(self.translation),
            rotation: Quat::from_array(self.rotation),
            scale: Vec3::from_array(self.scale),
        }
    }
}

impl Default for LocalTransform {
    /// Returns the identity transform.
    fn default() -> Self {
        Self::new(Transform::IDENTITY)
    }
}

/// A computed world matrix, preserving shear from scaled parent transforms.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform(pub Mat4);

/// An editable camera projected from an object's local negative Z axis.
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct SceneCamera {
    /// Whether this camera supplies the rendered view.
    pub active: bool,
    /// Whether projection is perspective rather than orthographic.
    pub perspective: bool,
    /// Vertical field of view in radians for perspective projection.
    pub fov_y: f32,
    /// Vertical extent for orthographic projection.
    pub height: f32,
    /// Near clip distance.
    pub near: f32,
    /// Far clip distance.
    pub far: f32,
    /// Radiance multiplier before tonemapping.
    pub exposure: f32,
}

impl Default for SceneCamera {
    /// Creates an active perspective camera with the engine's default lens.
    fn default() -> Self {
        Self {
            active: true,
            perspective: true,
            fov_y: std::f32::consts::FRAC_PI_3,
            height: 10.0,
            near: 0.05,
            far: 500.0,
            exposure: 1.0,
        }
    }
}

impl SceneCamera {
    /// Constructs a renderer-independent camera at the supplied world matrix.
    pub fn camera(&self, world: Mat4) -> fr_camera::Camera {
        let projection = if self.perspective {
            fr_camera::Projection::Perspective(fr_camera::PerspectiveProjection {
                fov_y: self.fov_y,
                near: self.near,
                far: self.far,
            })
        } else {
            fr_camera::Projection::Orthographic(fr_camera::OrthographicProjection {
                height: self.height,
                near: self.near,
                far: self.far,
            })
        };
        fr_camera::Camera {
            position: world.transform_point3(Vec3::ZERO),
            target: world.transform_point3(Vec3::NEG_Z),
            up: world.transform_vector3(Vec3::Y),
            projection,
            exposure: self.exposure,
        }
    }
}

/// An editable light placed by an object's transform.
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub enum SceneLight {
    /// An infinitely distant light directed along local negative Z.
    Directional {
        /// Linear RGB colour.
        color: [f32; 3],
        /// Illuminance on a facing surface.
        intensity: f32,
        /// Whether this light casts shadows.
        shadows: bool,
        /// The distance from the camera covered by shadows.
        shadow_distance: f32,
    },
    /// A light emitting equally in every direction.
    Point {
        /// Linear RGB colour.
        color: [f32; 3],
        /// Illuminance at one unit of distance.
        intensity: f32,
        /// The falloff range; zero means unlimited.
        range: f32,
        /// Whether this light casts shadows.
        shadows: bool,
    },
    /// A cone of light directed along local negative Z.
    Spot {
        /// Linear RGB colour.
        color: [f32; 3],
        /// Illuminance at one unit of distance.
        intensity: f32,
        /// The falloff range; zero means unlimited.
        range: f32,
        /// The inner cone half-angle in radians.
        inner_angle: f32,
        /// The outer cone half-angle in radians.
        outer_angle: f32,
        /// Whether this light casts shadows.
        shadows: bool,
    },
}

impl SceneLight {
    /// Converts authored values to a world-space engine light.
    pub fn light(&self, world: Mat4) -> fr_light::Light {
        let position = world.transform_point3(Vec3::ZERO);
        let direction = world.transform_vector3(Vec3::NEG_Z);
        match *self {
            Self::Directional {
                color,
                intensity,
                shadows,
                shadow_distance,
            } => fr_light::DirectionalLight {
                direction,
                color: Vec3::from_array(color),
                intensity,
                cast_shadows: shadows,
                shadow_distance,
            }
            .into(),
            Self::Point {
                color,
                intensity,
                range,
                shadows,
            } => fr_light::PointLight {
                position,
                color: Vec3::from_array(color),
                intensity,
                range,
                cast_shadows: shadows,
            }
            .into(),
            Self::Spot {
                color,
                intensity,
                range,
                inner_angle,
                outer_angle,
                shadows,
            } => fr_light::SpotLight {
                position,
                direction,
                color: Vec3::from_array(color),
                intensity,
                range,
                inner_angle,
                outer_angle,
                cast_shadows: shadows,
            }
            .into(),
        }
    }
}
