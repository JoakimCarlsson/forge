//! Frame timing and shared value types of the forge engine.
//!
//! This crate has no window or graphics dependency, so everything in it is
//! testable headlessly. It owns the one set of math types (glam's vectors,
//! quaternions and matrices, plus [`Transform`]) and the resource handles
//! ([`MeshId`], [`MaterialId`], [`TextureId`]) that the crates above share.

mod clock;
mod color;
mod fixed_step;
mod handles;
mod light;
mod projection;
mod skeleton;
mod transform;

pub use clock::FrameClock;
pub use color::srgb_to_linear;
pub use fixed_step::FixedStepper;
pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
pub use handles::{MaterialId, MeshId, TextureId};
pub use light::{DirectionalLight, Light, PointLight, SpotLight};
pub use projection::{look_at, orthographic, perspective};
pub use skeleton::{Bone, Skeleton, SkeletonError, humanoid};
pub use transform::Transform;
