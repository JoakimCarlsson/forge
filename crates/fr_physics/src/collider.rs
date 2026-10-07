//! The collider types, one per shape like Unity's collider set: [`SphereCollider`],
//! [`CapsuleCollider`], [`BoxCollider`] and the general [`HullCollider`].
//!
//! Each owns its constructors and the shape specific code: mass properties, bounds, reach and ray
//! casts. All convert into the [`Geometry`](crate::geometry::Geometry) enum the shapes of a world
//! and the narrow phase work with.

mod box_collider;
mod capsule;
mod hull;
mod sphere;

pub use box_collider::BoxCollider;
pub use capsule::CapsuleCollider;
pub use hull::HullCollider;
pub use sphere::SphereCollider;
