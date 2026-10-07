//! How a camera maps view space into clip space.

use fr_math::{Mat4, orthographic, perspective};

/// The default vertical field of view, in radians.
const DEFAULT_FOV_Y: f32 = std::f32::consts::FRAC_PI_3;

/// The default distance to the near clip plane.
const DEFAULT_NEAR: f32 = 0.05;

/// The default distance to the far clip plane.
const DEFAULT_FAR: f32 = 500.0;

/// The default vertical extent of an orthographic view, in world units.
const DEFAULT_HEIGHT: f32 = 10.0;

/// A projection with perspective foreshortening.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveProjection {
    /// The vertical field of view in radians.
    pub fov_y: f32,
    /// The distance to the near clip plane.
    pub near: f32,
    /// The distance to the far clip plane.
    pub far: f32,
}

impl Default for PerspectiveProjection {
    /// A 60 degree field of view between 0.05 and 500 units.
    fn default() -> Self {
        Self {
            fov_y: DEFAULT_FOV_Y,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
        }
    }
}

/// A projection that keeps parallel lines parallel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrthographicProjection {
    /// The vertical extent of the view in world units; the horizontal one follows the aspect.
    pub height: f32,
    /// The distance to the near clip plane.
    pub near: f32,
    /// The distance to the far clip plane.
    pub far: f32,
}

impl Default for OrthographicProjection {
    /// Ten units tall between 0.05 and 500 units.
    fn default() -> Self {
        Self {
            height: DEFAULT_HEIGHT,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
        }
    }
}

/// Either kind of projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// A [`PerspectiveProjection`].
    Perspective(PerspectiveProjection),
    /// An [`OrthographicProjection`].
    Orthographic(OrthographicProjection),
}

impl Projection {
    /// The matrix taking view space into clip space for a viewport of `aspect` width over height.
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        let aspect = aspect.max(f32::EPSILON);
        match *self {
            Self::Perspective(projection) => {
                perspective(projection.fov_y, aspect, projection.near, projection.far)
            }
            Self::Orthographic(projection) => {
                let half_height = projection.height * 0.5;
                let half_width = half_height * aspect;
                orthographic(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    projection.near,
                    projection.far,
                )
            }
        }
    }

    /// The distance to the near clip plane.
    pub fn near(&self) -> f32 {
        match self {
            Self::Perspective(projection) => projection.near,
            Self::Orthographic(projection) => projection.near,
        }
    }

    /// The distance to the far clip plane.
    pub fn far(&self) -> f32 {
        match self {
            Self::Perspective(projection) => projection.far,
            Self::Orthographic(projection) => projection.far,
        }
    }

    /// The same projection clipped between `near` and `far`.
    pub fn with_clip(&self, near: f32, far: f32) -> Self {
        match *self {
            Self::Perspective(projection) => Self::Perspective(PerspectiveProjection {
                near,
                far,
                ..projection
            }),
            Self::Orthographic(projection) => Self::Orthographic(OrthographicProjection {
                near,
                far,
                ..projection
            }),
        }
    }
}

impl Default for Projection {
    /// The default perspective projection.
    fn default() -> Self {
        Self::Perspective(PerspectiveProjection::default())
    }
}

impl From<PerspectiveProjection> for Projection {
    /// Wraps a perspective projection.
    fn from(projection: PerspectiveProjection) -> Self {
        Self::Perspective(projection)
    }
}

impl From<OrthographicProjection> for Projection {
    /// Wraps an orthographic projection.
    fn from(projection: OrthographicProjection) -> Self {
        Self::Orthographic(projection)
    }
}
