//! The convex hull collider: any convex polyhedron, shared between the bodies that use it.

use std::ops::Deref;
use std::sync::Arc;

use fr_math::{Aabb, Vec3};

use crate::geometry::{Extent, Geometry, MassData, RayHit, ShapeProxy};
use crate::hull::Hull;
use crate::math::Pose;

/// A convex hull in the local frame, shared by every collider made from the same value.
#[derive(Clone, Debug)]
pub struct HullCollider {
    /// The shared hull.
    hull: Arc<Hull>,
}

impl HullCollider {
    /// A collider of `hull`.
    pub fn new(hull: Hull) -> Self {
        Self {
            hull: Arc::new(hull),
        }
    }

    /// A collider of the convex hull of `points`, or `None` when they do not span a volume.
    pub fn from_points(points: &[Vec3]) -> Option<Self> {
        Hull::from_points(points).map(Self::new)
    }

    /// A collider of a hull that is already shared.
    pub fn from_shared(hull: Arc<Hull>) -> Self {
        Self { hull }
    }

    /// The shared hull.
    pub fn shared(&self) -> &Arc<Hull> {
        &self.hull
    }

    /// The point cloud of the shape for distance queries.
    pub fn proxy(&self) -> ShapeProxy<'_> {
        ShapeProxy {
            points: self.hull.points(),
            radius: 0.0,
        }
    }

    /// The mass properties of the hull for `density`.
    pub fn compute_mass(&self, density: f32) -> MassData {
        MassData {
            mass: density * self.hull.volume(),
            center: self.hull.center(),
            inertia: self.hull.central_inertia() * density,
        }
    }

    /// The bounds of the hull moved by `pose`.
    pub fn compute_aabb(&self, pose: &Pose) -> Aabb {
        self.hull.compute_aabb(pose)
    }

    /// The reach of the hull about the local point `origin`.
    pub fn extent(&self, origin: Vec3) -> Extent {
        let mut max = Vec3::ZERO;
        for &point in self.hull.points() {
            max = max.max((point - origin).abs());
        }
        Extent {
            min: self.hull.inner_radius(),
            max,
        }
    }

    /// The distance from the centre of the hull to its farthest point, which sizes the bounds
    /// margin of the hull.
    pub fn margin_radius(&self) -> f32 {
        let mut max_squared = 0.0_f32;
        for &point in self.hull.points() {
            max_squared = max_squared.max((point - self.hull.center()).length_squared());
        }
        max_squared.sqrt()
    }

    /// The reference point whose distance bounds the radius of the hull for sweeps.
    pub fn local_centroid(&self) -> Vec3 {
        self.hull.center()
    }

    /// Casts a segment against the hull in its local frame.
    pub fn ray_cast(&self, origin: Vec3, translation: Vec3, max_fraction: f32) -> Option<RayHit> {
        let (fraction, normal) = self.hull.ray_cast(origin, translation, max_fraction)?;
        Some(RayHit {
            fraction,
            point: origin + translation * fraction,
            normal,
        })
    }
}

impl Deref for HullCollider {
    type Target = Hull;

    /// The hull itself.
    fn deref(&self) -> &Hull {
        &self.hull
    }
}

impl From<Hull> for HullCollider {
    /// A collider of the hull.
    fn from(hull: Hull) -> Self {
        Self::new(hull)
    }
}

impl From<HullCollider> for Geometry {
    /// The hull as the shape of a body.
    fn from(hull: HullCollider) -> Self {
        Self::Hull(hull)
    }
}
