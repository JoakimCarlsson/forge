//! The capsule collider: the points within a radius of a segment in the local frame.

use fr_math::{Aabb, Mat3, Quat, Vec3};
use std::f32::consts::PI;

use crate::collider::SphereCollider;
use crate::constants::LINEAR_SLOP;
use crate::geometry::{Extent, Geometry, MassData, RayHit, ShapeProxy};
use crate::math::{Pose, cylinder_inertia, length_and_normalize, rotate_inertia, sphere_inertia};

/// The set of points within `radius` of the segment between two centres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapsuleCollider {
    /// The two ends of the core segment in the local frame.
    points: [Vec3; 2],
    /// The radius.
    pub radius: f32,
}

impl CapsuleCollider {
    /// A capsule from the two ends of its core segment and its radius.
    pub const fn new(center1: Vec3, center2: Vec3, radius: f32) -> Self {
        Self {
            points: [center1, center2],
            radius,
        }
    }

    /// A capsule along the local y axis with `half_height` of core on each side of the origin.
    pub const fn along_y(half_height: f32, radius: f32) -> Self {
        Self::new(
            Vec3::new(0.0, -half_height, 0.0),
            Vec3::new(0.0, half_height, 0.0),
            radius,
        )
    }

    /// The first end of the core segment.
    pub const fn center1(&self) -> Vec3 {
        self.points[0]
    }

    /// The second end of the core segment.
    pub const fn center2(&self) -> Vec3 {
        self.points[1]
    }

    /// Both ends of the core segment.
    pub const fn centers(&self) -> &[Vec3; 2] {
        &self.points
    }

    /// The point cloud of the shape for distance queries.
    pub fn proxy(&self) -> ShapeProxy<'_> {
        ShapeProxy {
            points: self.centers(),
            radius: self.radius,
        }
    }

    /// The mass properties of the capsule for `density`.
    pub fn compute_mass(&self, density: f32) -> MassData {
        let c1 = self.center1();
        let c2 = self.center2();
        let r = self.radius;
        let cylinder_height = (c2 - c1).length();
        let cylinder_mass = PI * r * r * cylinder_height * density;
        let sphere_mass = (4.0 / 3.0) * PI * r * r * r * density;
        let mut inertia =
            cylinder_inertia(cylinder_mass, r, cylinder_height) + sphere_inertia(sphere_mass, r);
        let parallel = 0.125 * sphere_mass * (3.0 * r + 2.0 * cylinder_height) * cylinder_height;
        inertia.x_axis.x += parallel;
        inertia.z_axis.z += parallel;
        let mut rotation = Mat3::IDENTITY;
        if cylinder_height * cylinder_height > 1000.0 * f32::MIN_POSITIVE {
            let direction = (c2 - c1).normalize();
            rotation = Mat3::from_quat(Quat::from_rotation_arc(Vec3::Y, direction));
        }
        MassData {
            mass: sphere_mass + cylinder_mass,
            center: (c1 + c2) * 0.5,
            inertia: rotate_inertia(rotation, inertia),
        }
    }

    /// The bounds of the capsule moved by `pose`.
    pub fn compute_aabb(&self, pose: &Pose) -> Aabb {
        let a = pose.transform_point(self.center1());
        let b = pose.transform_point(self.center2());
        let r = Vec3::splat(self.radius);
        Aabb::new(a.min(b) - r, a.max(b) + r)
    }

    /// The reach of the capsule about the local point `origin`.
    pub fn extent(&self, origin: Vec3) -> Extent {
        let c1 = self.center1() - origin;
        let c2 = self.center2() - origin;
        Extent {
            min: self.radius,
            max: c1.abs().max(c2.abs()) + Vec3::splat(self.radius),
        }
    }

    /// The radius that sizes the bounds margin of the capsule.
    pub fn margin_radius(&self) -> f32 {
        0.5 * (self.center2() - self.center1()).length() + self.radius
    }

    /// The reference point whose distance bounds the radius of the capsule for sweeps.
    pub fn local_centroid(&self) -> Vec3 {
        (self.center1() + self.center2()) * 0.5
    }

    /// Casts a segment against the capsule in its local frame; a segment that starts inside hits
    /// at fraction zero.
    pub fn ray_cast(&self, origin: Vec3, translation: Vec3, max_fraction: f32) -> Option<RayHit> {
        let c1 = self.center1();
        let c2 = self.center2();
        let r = self.radius;
        let d = c2 - c1;
        let tol = 0.01 * LINEAR_SLOP;
        let length_squared = d.length_squared();
        let sphere_at = |center: Vec3| SphereCollider { center, radius: r };
        if length_squared < tol * tol {
            return sphere_at((c1 + c2) * 0.5).ray_cast(origin, translation, max_fraction);
        }
        let s = origin - c1;
        let length = length_squared.sqrt();
        let axis = d * (1.0 / length);
        let u = s.dot(axis);
        let sc = s - axis * u;
        let sc2 = sc.length_squared();
        if sc2 < r * r {
            let u_clamped = u.clamp(0.0, length);
            let scp = s - axis * u_clamped;
            if scp.length_squared() < r * r {
                return Some(RayHit {
                    fraction: 0.0,
                    point: origin,
                    normal: Vec3::ZERO,
                });
            }
            return sphere_at(c1 + axis * u_clamped).ray_cast(origin, translation, max_fraction);
        }
        let (ray_length, ray_axis) = length_and_normalize(translation);
        if ray_length == 0.0 {
            return None;
        }
        let v = u + max_fraction * translation.dot(axis);
        if (u < -r && v < -r) || (length + r < u && length + r < v) {
            return None;
        }
        let a12 = axis.dot(ray_axis);
        let det = 1.0 - a12 * a12;
        let tr = if det < f32::EPSILON {
            let perp = ray_axis - axis * a12;
            let perp2 = perp.length_squared();
            let beta = sc.dot(perp);
            let gamma = sc2 - r * r;
            let disc = beta * beta - perp2 * gamma;
            if beta >= 0.0 || disc < 0.0 {
                return None;
            }
            gamma / (-beta + disc.sqrt())
        } else {
            let inv_det = 1.0 / det;
            let sa1 = u;
            let sa2 = s.dot(ray_axis);
            let t1 = (sa1 - a12 * sa2) * inv_det;
            let t2 = (a12 * sa1 - sa2) * inv_det;
            let p1 = axis * t1;
            let p2 = s + ray_axis * t2;
            let g = p2 - p1;
            let g2 = g.length_squared();
            if g2 > r * r {
                return None;
            }
            t2 - ((r * r - g2) * inv_det).sqrt()
        };
        if tr < 0.0 || max_fraction * ray_length < tr {
            return None;
        }
        let tc = u + tr * a12;
        if tc < 0.0 {
            return sphere_at(c1).ray_cast(origin, translation, max_fraction);
        }
        if length < tc {
            return sphere_at(c2).ray_cast(origin, translation, max_fraction);
        }
        let p = s + ray_axis * tr;
        let normal = crate::math::normalize(p - axis * tc);
        Some(RayHit {
            fraction: (tr / ray_length).clamp(0.0, max_fraction),
            point: c1 + p,
            normal,
        })
    }
}

impl From<CapsuleCollider> for Geometry {
    /// The capsule as the shape of a body.
    fn from(capsule: CapsuleCollider) -> Self {
        Self::Capsule(capsule)
    }
}
