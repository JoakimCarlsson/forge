//! CPU-side asset data and the importers that produce it.
//!
//! Nothing here touches a graphics device: a [`ModelData`] is plain vectors of
//! vertices, pixels and material factors that a renderer uploads. Depends on
//! `fr_core` only.
//!
//! The crate root is the facade and nothing else.

mod error;
mod gltf_import;
mod image;
mod material;
mod mesh;
mod model;
mod primitives;

pub use error::AssetError;
pub use gltf_import::load_gltf;
pub use image::{Filter, ImageData, SamplerData, TextureData, Wrap};
pub use material::{AlphaMode, MaterialData, TextureSlot};
pub use mesh::MeshData;
pub use model::{ModelData, ModelPart};
pub use primitives::{cube, plane, sphere};
