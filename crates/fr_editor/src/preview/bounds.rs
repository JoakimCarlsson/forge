//! The extents of authored nodes in the world, which picking and framing read.

use fr_document::Guid;
use fr_math::{Aabb, Ray3d, Vec3};
use fr_transform::Transform;

/// The half size of the marker box a node without any extent is given, so an
/// empty entity can still be picked.
const MARKER_HALF_SIZE: f32 = 0.15;

/// A box in the local space of a transform: it stays a box when the transform
/// rotates, so a rotated cube is picked as a cube and outlined as one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrientedBox {
    /// The box in local space.
    pub local: Aabb,
    /// The placement of the local space in the world.
    pub transform: Transform,
}

impl OrientedBox {
    /// The eight corners in the world; bit 0 of the index selects the maximum
    /// x, bit 1 the maximum y and bit 2 the maximum z.
    pub fn corners(&self) -> [Vec3; 8] {
        let matrix = self.transform.matrix();
        std::array::from_fn(|index| {
            let pick = |bit: usize, low: f32, high: f32| {
                if index & bit == 0 { low } else { high }
            };
            matrix.transform_point3(Vec3::new(
                pick(1, self.local.min.x, self.local.max.x),
                pick(2, self.local.min.y, self.local.max.y),
                pick(4, self.local.min.z, self.local.max.z),
            ))
        })
    }

    /// The axis aligned box around the corners in the world.
    pub fn world_aabb(&self) -> Aabb {
        let corners = self.corners();
        corners
            .iter()
            .fold(Aabb::new(corners[0], corners[0]), |bounds, corner| {
                Aabb::new(bounds.min.min(*corner), bounds.max.max(*corner))
            })
    }

    /// The distance along `ray` at which it enters the box, when it does.
    pub fn intersect(&self, ray: &Ray3d) -> Option<f32> {
        let inverse = self.transform.matrix().inverse();
        let origin = inverse.transform_point3(ray.origin);
        let direction = inverse.transform_vector3(ray.direction.as_vec3());
        if !(origin.is_finite() && direction.is_finite()) {
            return None;
        }
        self.local.ray_entry(origin, direction, f32::INFINITY)
    }
}

/// The extent of one node in the world: the boxes of the components of the
/// node and of everything inside it, when it is a prefab instance.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeBounds {
    /// The authored node.
    pub node: Guid,
    /// The axis aligned box around every part.
    pub world: Aabb,
    /// The boxes the node is picked and outlined by.
    pub parts: Vec<OrientedBox>,
}

impl NodeBounds {
    /// The bounds of a node from its parts; none without any.
    pub fn from_parts(node: Guid, parts: Vec<OrientedBox>) -> Option<Self> {
        let world = parts
            .iter()
            .map(OrientedBox::world_aabb)
            .reduce(|a, b| a.union(&b))?;
        Some(Self { node, world, parts })
    }

    /// The bounds of a node that has no extent: a small box at its placement.
    pub fn marker(node: Guid, transform: Transform) -> Self {
        let part = OrientedBox {
            local: Aabb::from_center_extents(Vec3::ZERO, Vec3::splat(MARKER_HALF_SIZE)),
            transform: Transform {
                scale: Vec3::ONE,
                ..transform
            },
        };
        Self {
            node,
            world: part.world_aabb(),
            parts: vec![part],
        }
    }

    /// The eight corners of the box the node is outlined by: its own oriented
    /// box when it has one part, otherwise the axis aligned box around all of
    /// them.
    pub fn outline(&self) -> [Vec3; 8] {
        match self.parts.as_slice() {
            [part] => part.corners(),
            _ => OrientedBox {
                local: self.world,
                transform: Transform::IDENTITY,
            }
            .corners(),
        }
    }

    /// The distance along `ray` at which it first enters one of the parts.
    pub fn intersect(&self, ray: &Ray3d) -> Option<f32> {
        self.parts
            .iter()
            .filter_map(|part| part.intersect(ray))
            .reduce(f32::min)
    }
}
