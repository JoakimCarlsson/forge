//! The one math crate of the forge engine.
//!
//! glam's vectors, quaternions and matrices are re-exported as the shared
//! vocabulary, next to the value types built on them: [`Dir3`], [`Ray3d`],
//! [`Plane`], [`Aabb`], the logical-pixel [`Rect`], and the view and
//! projection matrices of [`projection`]. Rays carry their own intersection
//! tests against planes, spheres, boxes and triangles; what a ray is cast
//! against in a physics world or through a camera lives in the crates that
//! own those.
//!
//! The crate root is the facade and nothing else.

mod aabb;
mod direction;
mod plane;
mod projection;
mod ray;
mod rect;

pub use aabb::Aabb;
pub use direction::Dir3;
pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
pub use plane::Plane;
pub use projection::{look_at, orthographic, perspective};
pub use ray::Ray3d;
pub use rect::{Point, Rect, Size};
