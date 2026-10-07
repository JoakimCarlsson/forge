//! The shared description of a collider's shape: the [`Geometry`] enum over the collider types
//! of [`crate::collider`], with the mass, bounds, extents and ray hit types they report.

use fr_math::{Aabb, Mat3, Vec3};

use crate::collider::{BoxCollider, CapsuleCollider, HullCollider, SphereCollider};
use crate::math::{Pose, steiner};

/// The mass, centre and rotational inertia about the centre of a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassData {
    /// The mass.
    pub mass: f32,
    /// The centre of mass in the local frame.
    pub center: Vec3,
    /// The inertia tensor about the centre of mass.
    pub inertia: Mat3,
}

/// The point cloud and radius a distance query sees: the shape is the cloud's convex hull
/// grown by the radius.
#[derive(Clone, Copy, Debug)]
pub struct ShapeProxy<'a> {
    /// The points whose convex hull is the core.
    pub points: &'a [Vec3],
    /// How far the shape reaches beyond the core.
    pub radius: f32,
}

/// The nearest point where a ray enters a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// The fraction of the ray translation at the hit.
    pub fraction: f32,
    /// The hit point.
    pub point: Vec3,
    /// The surface normal at the hit.
    pub normal: Vec3,
}

/// How far a shape reaches about a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    /// The radius of the largest sphere about the point inside the shape.
    pub min: f32,
    /// The farthest reach along each axis.
    pub max: Vec3,
}

/// The shape of a collider: what the world and the narrow phase see of a [`SphereCollider`],
/// [`CapsuleCollider`], [`BoxCollider`] or [`HullCollider`].
#[derive(Clone, Debug)]
pub enum Geometry {
    /// A ball.
    Sphere(SphereCollider),
    /// A capsule.
    Capsule(CapsuleCollider),
    /// A convex hull, shared between colliders; boxes are hulls.
    Hull(HullCollider),
}

impl Geometry {
    /// A ball of `radius` centred on the local origin.
    pub fn sphere(radius: f32) -> Self {
        SphereCollider::new(radius).into()
    }

    /// A capsule along the local y axis with `half_height` of core on each side of the origin.
    pub fn capsule_y(half_height: f32, radius: f32) -> Self {
        CapsuleCollider::along_y(half_height, radius).into()
    }

    /// A box of half sizes `half_extents` centred on the local origin.
    pub fn cuboid(half_extents: Vec3) -> Self {
        BoxCollider::new(half_extents).into()
    }

    /// The point cloud of the shape for distance queries.
    pub fn proxy(&self) -> ShapeProxy<'_> {
        match self {
            Self::Sphere(sphere) => sphere.proxy(),
            Self::Capsule(capsule) => capsule.proxy(),
            Self::Hull(hull) => hull.proxy(),
        }
    }

    /// The mass properties of the shape for `density`.
    pub fn compute_mass(&self, density: f32) -> MassData {
        match self {
            Self::Sphere(sphere) => sphere.compute_mass(density),
            Self::Capsule(capsule) => capsule.compute_mass(density),
            Self::Hull(hull) => hull.compute_mass(density),
        }
    }

    /// The bounds of the shape moved by `pose`.
    pub fn compute_aabb(&self, pose: &Pose) -> Aabb {
        match self {
            Self::Sphere(sphere) => sphere.compute_aabb(pose),
            Self::Capsule(capsule) => capsule.compute_aabb(pose),
            Self::Hull(hull) => hull.compute_aabb(pose),
        }
    }

    /// The reach of the shape about the local point `origin`.
    pub fn extent(&self, origin: Vec3) -> Extent {
        match self {
            Self::Sphere(sphere) => sphere.extent(origin),
            Self::Capsule(capsule) => capsule.extent(origin),
            Self::Hull(hull) => hull.extent(origin),
        }
    }

    /// The distance from the local origin to the farthest point of the shape's centroid reach,
    /// which sizes the bounds margin of the shape.
    pub fn margin_radius(&self) -> f32 {
        match self {
            Self::Sphere(sphere) => sphere.margin_radius(),
            Self::Capsule(capsule) => capsule.margin_radius(),
            Self::Hull(hull) => hull.margin_radius(),
        }
    }

    /// The reference point whose distance bounds the radius of the shape for sweeps.
    pub fn local_centroid(&self) -> Vec3 {
        match self {
            Self::Sphere(sphere) => sphere.local_centroid(),
            Self::Capsule(capsule) => capsule.local_centroid(),
            Self::Hull(hull) => hull.local_centroid(),
        }
    }

    /// Casts a segment against the shape in its local frame.
    pub fn ray_cast(&self, origin: Vec3, translation: Vec3, max_fraction: f32) -> Option<RayHit> {
        match self {
            Self::Sphere(sphere) => sphere.ray_cast(origin, translation, max_fraction),
            Self::Capsule(capsule) => capsule.ray_cast(origin, translation, max_fraction),
            Self::Hull(hull) => hull.ray_cast(origin, translation, max_fraction),
        }
    }
}

/// The inertia of a solid of `mass` and central `inertia` seen from `origin` away from its centre.
pub fn inertia_about(mass: f32, inertia: Mat3, origin: Vec3) -> Mat3 {
    inertia + steiner(mass, origin)
}
