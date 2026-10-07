//! Punctual light sources: directional, point and spot.
//!
//! Colours are linear RGB. Intensities use photometric-flavoured units: a
//! directional light's intensity is the illuminance it gives a surface facing
//! it, and a point or spot light's is the illuminance it gives a surface one
//! unit away and facing it, falling with the square of the distance.

use glam::Vec3;

use crate::transform::Transform;

/// The default distance in front of the camera shadows are drawn to.
const DEFAULT_SHADOW_DISTANCE: f32 = 60.0;

/// A light infinitely far away, shining the same way everywhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalLight {
    /// The direction the light travels, which need not be normalized.
    pub direction: Vec3,
    /// The linear colour of the light.
    pub color: Vec3,
    /// The illuminance of a surface facing the light; around 3 is a bright day.
    pub intensity: f32,
    /// Whether objects block this light; only the first such light in a scene does.
    pub cast_shadows: bool,
    /// How far from the camera shadows reach, in world units.
    pub shadow_distance: f32,
}

impl Default for DirectionalLight {
    /// A white light shining down and a little to the side, casting shadows.
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.4, -1.0, -0.3),
            color: Vec3::ONE,
            intensity: 3.0,
            cast_shadows: true,
            shadow_distance: DEFAULT_SHADOW_DISTANCE,
        }
    }
}

/// A light shining equally in every direction from one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    /// Where the light is, in world space.
    pub position: Vec3,
    /// The linear colour of the light.
    pub color: Vec3,
    /// The illuminance at one unit of distance.
    pub intensity: f32,
    /// The distance beyond which the light is fully faded out; zero means it never is.
    pub range: f32,
    /// Whether objects block this light, at the cost of six shadow maps.
    pub cast_shadows: bool,
}

impl Default for PointLight {
    /// A white light at the origin reaching ten units.
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            color: Vec3::ONE,
            intensity: 10.0,
            range: 10.0,
            cast_shadows: false,
        }
    }
}

/// A light shining in a cone from one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpotLight {
    /// Where the light is, in world space.
    pub position: Vec3,
    /// The direction the cone points, which need not be normalized.
    pub direction: Vec3,
    /// The linear colour of the light.
    pub color: Vec3,
    /// The illuminance at one unit of distance along the axis.
    pub intensity: f32,
    /// The distance beyond which the light is fully faded out; zero means it never is.
    pub range: f32,
    /// The half-angle in radians inside which the light is at full strength.
    pub inner_angle: f32,
    /// The half-angle in radians beyond which there is no light.
    pub outer_angle: f32,
    /// Whether objects block this light, at the cost of one shadow map.
    pub cast_shadows: bool,
}

impl Default for SpotLight {
    /// A white light at the origin pointing down its cone of 20 to 30 degrees.
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            direction: Vec3::NEG_Y,
            color: Vec3::ONE,
            intensity: 30.0,
            range: 20.0,
            inner_angle: 0.35,
            outer_angle: 0.52,
            cast_shadows: false,
        }
    }
}

/// One light of any kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Light {
    /// A [`DirectionalLight`].
    Directional(DirectionalLight),
    /// A [`PointLight`].
    Point(PointLight),
    /// A [`SpotLight`].
    Spot(SpotLight),
}

impl Light {
    /// This light placed inside `parent`: positions are moved, turned and
    /// scaled with it, directions are turned, and ranges follow the largest scale.
    pub fn placed(&self, parent: &Transform) -> Self {
        let matrix = parent.matrix();
        let reach = parent.scale.abs().max_element();
        match *self {
            Self::Directional(light) => Self::Directional(DirectionalLight {
                direction: parent.rotation * light.direction,
                ..light
            }),
            Self::Point(light) => Self::Point(PointLight {
                position: matrix.transform_point3(light.position),
                range: light.range * reach,
                ..light
            }),
            Self::Spot(light) => Self::Spot(SpotLight {
                position: matrix.transform_point3(light.position),
                direction: parent.rotation * light.direction,
                range: light.range * reach,
                ..light
            }),
        }
    }
}

impl From<DirectionalLight> for Light {
    /// Wraps a directional light.
    fn from(light: DirectionalLight) -> Self {
        Self::Directional(light)
    }
}

impl From<PointLight> for Light {
    /// Wraps a point light.
    fn from(light: PointLight) -> Self {
        Self::Point(light)
    }
}

impl From<SpotLight> for Light {
    /// Wraps a spot light.
    fn from(light: SpotLight) -> Self {
        Self::Spot(light)
    }
}
