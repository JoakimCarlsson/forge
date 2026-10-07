//! Planes in three dimensions.

use glam::Vec3;

/// A plane with unit `normal` through the points `p` with `dot(normal, p) == offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// The unit normal.
    pub normal: Vec3,
    /// The signed distance of the plane from the origin along the normal.
    pub offset: f32,
}

impl Plane {
    /// The plane with unit `normal` through `point`.
    pub fn from_normal_and_point(normal: Vec3, point: Vec3) -> Self {
        Self {
            normal,
            offset: normal.dot(point),
        }
    }

    /// The signed distance of `point` in front of the plane.
    pub fn separation(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.offset
    }
}
