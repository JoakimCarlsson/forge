//! A direction in three dimensions: a vector known to have unit length.

use std::ops::{Deref, Mul, Neg};

use glam::Vec3;

/// A [`Vec3`] of unit length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dir3(Vec3);

impl Dir3 {
    /// The positive x axis.
    pub const X: Self = Self(Vec3::X);
    /// The positive y axis.
    pub const Y: Self = Self(Vec3::Y);
    /// The positive z axis.
    pub const Z: Self = Self(Vec3::Z);
    /// The negative x axis.
    pub const NEG_X: Self = Self(Vec3::NEG_X);
    /// The negative y axis.
    pub const NEG_Y: Self = Self(Vec3::NEG_Y);
    /// The negative z axis.
    pub const NEG_Z: Self = Self(Vec3::NEG_Z);

    /// The direction of `vector`, or none when it is too short or not finite.
    pub fn new(vector: Vec3) -> Option<Self> {
        vector.try_normalize().map(Self)
    }

    /// Wraps `vector`, which the caller guarantees to be of unit length.
    pub const fn new_unchecked(vector: Vec3) -> Self {
        Self(vector)
    }

    /// The direction as a plain vector.
    pub const fn as_vec3(self) -> Vec3 {
        self.0
    }
}

impl Deref for Dir3 {
    type Target = Vec3;

    /// The unit vector.
    fn deref(&self) -> &Vec3 {
        &self.0
    }
}

impl From<Dir3> for Vec3 {
    /// The unit vector.
    fn from(direction: Dir3) -> Self {
        direction.0
    }
}

impl Neg for Dir3 {
    type Output = Self;

    /// The opposite direction.
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl Mul<f32> for Dir3 {
    type Output = Vec3;

    /// The unit vector scaled by `scale`.
    fn mul(self, scale: f32) -> Vec3 {
        self.0 * scale
    }
}
