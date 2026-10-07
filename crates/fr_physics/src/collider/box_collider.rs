//! The box collider: a cuboid built as a convex hull, optionally moved and turned in the local
//! frame.

use fr_math::{Quat, Vec3};

use crate::collider::HullCollider;
use crate::geometry::Geometry;
use crate::hull::Hull;
use crate::math::Pose;

/// A box of half sizes `half_extents` whose centre sits at `center` and whose axes are turned by
/// `rotation` in the local frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxCollider {
    /// Half the size along each axis of the box.
    pub half_extents: Vec3,
    /// The centre of the box in the local frame.
    pub center: Vec3,
    /// The orientation of the box in the local frame.
    pub rotation: Quat,
}

impl BoxCollider {
    /// A box of half sizes `half_extents` centred on the local origin with its axes along the
    /// local axes.
    pub const fn new(half_extents: Vec3) -> Self {
        Self {
            half_extents,
            center: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }

    /// The same box centred on `center` in the local frame.
    pub const fn with_center(mut self, center: Vec3) -> Self {
        self.center = center;
        self
    }

    /// The same box turned by `rotation` about its centre.
    pub const fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    /// The same box placed by `offset` in the local frame.
    pub const fn with_offset(self, offset: Pose) -> Self {
        self.with_center(offset.position)
            .with_rotation(offset.rotation)
    }

    /// The box as a convex hull.
    pub fn to_hull(&self) -> Hull {
        let hull = Hull::cuboid(self.half_extents);
        if self.center == Vec3::ZERO && self.rotation == Quat::IDENTITY {
            hull
        } else {
            hull.transformed(&Pose::new(self.center, self.rotation))
        }
    }
}

impl From<BoxCollider> for HullCollider {
    /// The box as a hull collider.
    fn from(collider: BoxCollider) -> Self {
        Self::new(collider.to_hull())
    }
}

impl From<BoxCollider> for Geometry {
    /// The box as the shape of a body.
    fn from(collider: BoxCollider) -> Self {
        Self::Hull(collider.into())
    }
}
