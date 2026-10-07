//! The mesh renderer subsystem: picking bounds from the model or the primitive,
//! and the Cube, Sphere and Capsule entries of the Create menu.

mod subsystem;

pub use subsystem::{primitive_entry, register};
