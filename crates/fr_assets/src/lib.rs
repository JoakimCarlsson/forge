//! Loaders of the forge engine: files in, CPU-side data out.
//!
//! Nothing here touches a graphics device: a [`ModelData`] gathers the
//! meshes, images, materials and lights of a file as the data types of
//! `fr_mesh`, `fr_image`, `fr_material` and `fr_light` for a renderer to
//! upload, and [`load_skin`] reads a skin as an `fr_transform` hierarchy with an
//! `fr_mesh` skin over it.
//!
//! The crate root is the facade and nothing else.

mod error;
mod gltf_import;
mod model;
mod text;

pub use error::{AssetError, SkinImportError};
pub use gltf_import::{load_gltf, load_skin, skin_from_gltf};
pub use model::{ModelData, ModelPart, SkinData};
pub use text::{load_text, save_text};
