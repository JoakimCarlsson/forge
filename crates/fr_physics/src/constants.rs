//! Tolerances and limits shared by the collision and solver code, in metres.

use std::f32::consts::PI;

/// A small length used as a collision and constraint tolerance.
pub const LINEAR_SLOP: f32 = 0.005;

/// The distance at which separated shapes still produce speculative contact points.
pub const SPECULATIVE_DISTANCE: f32 = 4.0 * LINEAR_SLOP;

/// The distance below which two shapes count as overlapped; GJK can return small
/// positive values for overlapped shapes in degenerate configurations.
pub const OVERLAP_SLOP: f32 = 0.1 * LINEAR_SLOP;

/// The largest rotation of a body per time step, in radians.
pub const MAX_ROTATION: f32 = 0.25 * PI;

/// The largest amount a proxy's bounds are fattened in the broad phase.
pub const MAX_AABB_MARGIN: f32 = 0.05;

/// The fraction of a shape's extent used as its bounds margin before the cap applies.
pub const AABB_MARGIN_FRACTION: f32 = 0.125;

/// The time a body must be still before it can fall asleep, in seconds.
pub const TIME_TO_SLEEP: f32 = 0.5;

/// The largest number of restitution iterations.
pub const MAX_RESTITUTION_ITERATIONS: u32 = 63;

/// The largest number of contact points between two touching shapes.
pub const MAX_MANIFOLD_POINTS: usize = 4;

/// The shortest capsule axis the manifold code handles; shorter capsules should be spheres.
pub const MIN_CAPSULE_LENGTH: f32 = LINEAR_SLOP;

/// The lower bound of the friction weight of a speculative contact point.
pub const MIN_FRICTION_WEIGHT: f32 = 1e-10;

/// The relative tolerance used to decide that two hull edges are parallel.
pub const PARALLEL_EDGE_TOL: f32 = 0.005;

/// The default distance a contact may be reused without a new manifold.
pub const CONTACT_RECYCLE_DISTANCE: f32 = 10.0 * LINEAR_SLOP;

/// The squared cosine of half the largest relative rotation that still recycles a contact.
pub const CONTACT_RECYCLE_ANGULAR_DISTANCE: f32 = 0.992_403_9;

/// The limit on positions and sizes, used to detect bad values.
pub const HUGE: f32 = 1.0e5;

/// The largest number of clipped polygon vertices.
pub const MAX_CLIP_POINTS: usize = 64;

/// The largest number of vertices, faces or edges of a convex hull.
pub const MAX_HULL_ELEMENTS: usize = 128;

/// A value that stands for no index in the packed hull and feature tables.
pub const NULL_INDEX: usize = usize::MAX;
