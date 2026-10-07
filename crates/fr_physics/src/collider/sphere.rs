//! The sphere collider: a ball with its centre in the local frame of the body.

use std::f32::consts::PI;

use fr_math::{Aabb, Vec3};

use crate::geometry::{Extent, Geometry, MassData, RayHit, ShapeProxy};
use crate::math::{Pose, length_and_normalize, sphere_inertia};

/// A ball.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereCollider {
    /// The centre in the local frame.
    pub center: Vec3,
    /// The radius.
    pub radius: f32,
}

impl SphereCollider {
    /// A ball of `radius` centred on the local origin.
    pub const fn new(radius: f32) -> Self {
        Self {
            center: Vec3::ZERO,
            radius,
        }
    }

    /// The same ball centred on `center` in the local frame.
    pub const fn with_center(mut self, center: Vec3) -> Self {
        self.center = center;
        self
    }

    /// The point cloud of the shape for distance queries.
    pub fn proxy(&self) -> ShapeProxy<'_> {
        ShapeProxy {
            points: std::slice::from_ref(&self.center),
            radius: self.radius,
        }
    }

    /// The mass properties of the ball for `density`.
    pub fn compute_mass(&self, density: f32) -> MassData {
        let radius = self.radius;
        let volume = 4.0 / 3.0 * PI * radius * radius * radius;
        let mass = volume * density;
        MassData {
            mass,
            center: self.center,
            inertia: sphere_inertia(mass, radius),
        }
    }

    /// The bounds of the ball moved by `pose`.
    pub fn compute_aabb(&self, pose: &Pose) -> Aabb {
        let center = pose.transform_point(self.center);
        Aabb::from_center_extents(center, Vec3::splat(self.radius))
    }

    /// The reach of the ball about the local point `origin`.
    pub fn extent(&self, origin: Vec3) -> Extent {
        Extent {
            min: self.radius,
            max: (self.center - origin).abs() + Vec3::splat(self.radius),
        }
    }

    /// The radius that sizes the bounds margin of the ball.
    pub fn margin_radius(&self) -> f32 {
        self.radius
    }

    /// The reference point whose distance bounds the radius of the ball for sweeps.
    pub fn local_centroid(&self) -> Vec3 {
        self.center
    }

    /// Casts a segment against the ball in its local frame; a segment that starts inside hits at
    /// fraction zero.
    pub fn ray_cast(&self, origin: Vec3, translation: Vec3, max_fraction: f32) -> Option<RayHit> {
        let s = origin - self.center;
        let rr = self.radius * self.radius;
        let inside = || {
            (s.length_squared() < rr).then_some(RayHit {
                fraction: 0.0,
                point: origin,
                normal: Vec3::ZERO,
            })
        };
        let (length, d) = length_and_normalize(translation);
        if length == 0.0 {
            return inside();
        }
        let t = -s.dot(d);
        let c = s + d * t;
        let cc = c.dot(c);
        if cc > rr {
            return None;
        }
        let h = (rr - cc).sqrt();
        let fraction = t - h;
        if fraction < 0.0 || max_fraction * length < fraction {
            return inside();
        }
        let hit_point = s + d * fraction;
        let normal = crate::math::normalize(hit_point);
        Some(RayHit {
            fraction: (fraction / length).min(max_fraction),
            point: self.center + normal * self.radius,
            normal,
        })
    }
}

impl From<SphereCollider> for Geometry {
    /// The ball as the shape of a body.
    fn from(sphere: SphereCollider) -> Self {
        Self::Sphere(sphere)
    }
}
