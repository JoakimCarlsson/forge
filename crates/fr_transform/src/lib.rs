//! Where an object sits in space: translation, rotation and scale, and trees of them.
//!
//! The crate root is the facade and nothing else.

mod hierarchy;
mod transform;

pub use hierarchy::{Hierarchy, HierarchyError, Node};
pub use transform::Transform;
