//! CPU-side metallic-roughness material data of the forge engine.
//!
//! A [`MaterialData`] is plain factors and texture slots that a renderer
//! uploads and names with a [`MaterialId`].
//!
//! The crate root is the facade and nothing else.

mod material;

pub use material::{AlphaMode, MaterialData, MaterialId, TextureSlot};
