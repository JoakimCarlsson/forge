//! The volume of space a camera sees.

use fr_math::{Aabb, Mat4, Plane, Vec3};

/// The six planes bounding what a camera sees, with their normals pointing inwards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    /// The left, right, bottom, top, near and far planes.
    pub planes: [Plane; 6],
}

impl Frustum {
    /// The frustum of a combined view and projection matrix whose clip depth runs from zero to one.
    pub fn from_view_projection(view_projection: &Mat4) -> Self {
        let rows = [
            view_projection.row(0),
            view_projection.row(1),
            view_projection.row(2),
            view_projection.row(3),
        ];
        let coefficients = [
            rows[3] + rows[0],
            rows[3] - rows[0],
            rows[3] + rows[1],
            rows[3] - rows[1],
            rows[2],
            rows[3] - rows[2],
        ];
        Self {
            planes: coefficients.map(|row| {
                let normal = row.truncate();
                let length = normal.length();
                Plane {
                    normal: normal / length,
                    offset: -row.w / length,
                }
            }),
        }
    }

    /// Whether `point` lies inside or on the frustum.
    pub fn contains_point(&self, point: Vec3) -> bool {
        self.planes
            .iter()
            .all(|plane| plane.separation(point) >= 0.0)
    }

    /// Whether the sphere of `radius` around `center` touches the frustum, conservatively.
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        self.planes
            .iter()
            .all(|plane| plane.separation(center) >= -radius)
    }

    /// Whether `aabb` touches the frustum, conservatively.
    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        self.planes.iter().all(|plane| {
            let nearest = Vec3::select(plane.normal.cmpge(Vec3::ZERO), aabb.max, aabb.min);
            plane.separation(nearest) >= 0.0
        })
    }
}
