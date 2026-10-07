//! Colliders: a geometry on a body with a material, a collision filter and an optional sensor
//! flag.

use fr_math::Vec3;

use crate::body::BodyId;
use crate::constants::{AABB_MARGIN_FRACTION, MAX_AABB_MARGIN};
use crate::geometry::Geometry;
use crate::slot::Handle;
use crate::tree::NodeId;
use fr_math::Aabb;

/// Marks [`ShapeId`].
#[derive(Clone, Copy, Debug)]
pub struct ShapeTag;

/// A handle to a collider of a world.
pub type ShapeId = Handle<ShapeTag>;

/// Decides which shapes may collide: by category and mask bits, and by group.
///
/// Two shapes collide when each one's mask accepts the other's category. Shapes of the same
/// nonzero group always collide when the group is positive and never when it is negative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filter {
    /// The bits naming the kind of this shape.
    pub category: u64,
    /// The categories this shape collides with.
    pub mask: u64,
    /// The group that overrides category and mask when two shapes share it.
    pub group: i32,
}

impl Filter {
    /// Whether shapes with the filters `self` and `other` may collide.
    pub fn should_collide(&self, other: &Self) -> bool {
        if self.group == other.group && self.group != 0 {
            return self.group > 0;
        }
        (self.mask & other.category) != 0 && (self.category & other.mask) != 0
    }
}

impl Default for Filter {
    /// Category one, colliding with everything, no group.
    fn default() -> Self {
        Self {
            category: 1,
            mask: u64::MAX,
            group: 0,
        }
    }
}

/// The surface of a collider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// The Coulomb friction coefficient, mixed between shapes by the geometric mean.
    pub friction: f32,
    /// The bounciness in zero to one, mixed between shapes by the maximum.
    pub restitution: f32,
    /// The resistance to rolling, as a fraction of the normal force times the radius.
    pub rolling_resistance: f32,
    /// A velocity the surface imparts on touching shapes, in the frame of the body, for
    /// conveyors.
    pub tangent_velocity: Vec3,
}

impl Default for Material {
    /// Friction 0.6, no restitution, rolling resistance or surface velocity.
    fn default() -> Self {
        Self {
            friction: 0.6,
            restitution: 0.0,
            rolling_resistance: 0.0,
            tangent_velocity: Vec3::ZERO,
        }
    }
}

/// The description of a collider to attach to a body.
#[derive(Clone, Debug)]
pub struct ShapeDef {
    /// The shape of the collider in the frame of the body.
    pub geometry: Geometry,
    /// The mass per unit volume in kilograms per cubic metre; the density of water by default.
    pub density: f32,
    /// The surface of the collider.
    pub material: Material,
    /// The collision filter.
    pub filter: Filter,
    /// A sensor detects overlaps but does not collide and has no mass.
    pub is_sensor: bool,
    /// Whether the shape is reported by sensors; true by default.
    pub enable_sensor_events: bool,
    /// Whether begin and end touch events are reported for contacts of the shape.
    pub enable_contact_events: bool,
    /// Whether attaching the shape updates the mass of the body; true by default.
    pub update_body_mass: bool,
    /// A value the game attaches to the shape.
    pub user_data: u64,
}

impl ShapeDef {
    /// A collider of `geometry` with default density, material and filter.
    pub fn new(geometry: Geometry) -> Self {
        Self {
            geometry,
            density: 1000.0,
            material: Material::default(),
            filter: Filter::default(),
            is_sensor: false,
            enable_sensor_events: true,
            enable_contact_events: false,
            update_body_mass: true,
            user_data: 0,
        }
    }

    /// The same collider with `density`.
    pub fn with_density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    /// The same collider with `material`.
    pub fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    /// The same collider with `filter`.
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }

    /// The same collider as a sensor.
    pub fn as_sensor(mut self) -> Self {
        self.is_sensor = true;
        self
    }
}

/// A collider of a world.
#[derive(Clone, Debug)]
pub(crate) struct Shape {
    /// The body the collider is attached to.
    pub(crate) body: BodyId,
    /// The shape in the frame of the body.
    pub(crate) geometry: Geometry,
    /// The mass per unit volume.
    pub(crate) density: f32,
    /// The surface.
    pub(crate) material: Material,
    /// The collision filter.
    pub(crate) filter: Filter,
    /// Whether the shape is a sensor.
    pub(crate) is_sensor: bool,
    /// Whether sensors report the shape.
    pub(crate) enable_sensor_events: bool,
    /// Whether contacts of the shape produce touch events.
    pub(crate) enable_contact_events: bool,
    /// The game's value.
    pub(crate) user_data: u64,
    /// The world bounds including the speculative margin.
    pub(crate) aabb: Aabb,
    /// The bounds stored in the broad phase, fattened by the margin.
    pub(crate) fat_aabb: Aabb,
    /// The fattening margin of the shape.
    pub(crate) aabb_margin: f32,
    /// The leaf of the shape in its broad phase tree.
    pub(crate) proxy: NodeId,
    /// Whether the shape is in the move buffer.
    pub(crate) moved: bool,
    /// The shapes a sensor overlapped at the end of the last step, sorted by handle.
    pub(crate) sensor_overlaps: Vec<ShapeId>,
}

impl Shape {
    /// The margin the broad phase fattens the bounds of `geometry` by.
    pub(crate) fn margin_of(geometry: &Geometry) -> f32 {
        MAX_AABB_MARGIN.min(AABB_MARGIN_FRACTION * geometry.margin_radius())
    }
}
