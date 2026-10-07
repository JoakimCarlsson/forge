//! Bone hierarchies of the forge engine and the pose math over them.
//!
//! The crate root is the facade and nothing else.

mod skeleton;

pub use skeleton::{Bone, Skeleton, SkeletonError, humanoid};
