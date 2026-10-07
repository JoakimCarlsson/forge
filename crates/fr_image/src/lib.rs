//! CPU-side image data and the sampling state a texture is read with.
//!
//! Nothing here touches a graphics device: an [`ImageData`] is plain pixels
//! that a renderer uploads and names with a [`TextureId`].
//!
//! The crate root is the facade and nothing else.

mod image;

pub use image::{Filter, ImageData, SamplerData, TextureData, TextureId, Wrap};
