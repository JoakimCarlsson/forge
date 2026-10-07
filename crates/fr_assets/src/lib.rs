//! Loaders of the forge engine: files in, CPU-side data out.
//!
//! Nothing here touches a graphics device: a [`ModelData`] gathers the
//! meshes, images, materials and lights of a file as the data types of
//! `fr_mesh`, `fr_image`, `fr_material` and `fr_light` for a renderer to
//! upload, and [`load_skeleton`] reads a skin as an `fr_skeleton` skeleton.
//!
//! [`AssetLibrary`] is the typed library over a project's asset root: models,
//! prefabs and scenes by identity or path, cached, with [`AssetLibrary::invalidate`]
//! as the hook a later hot reload drives.
//!
//! The crate root is the facade and nothing else.

mod error;
mod gltf_import;
mod library;
mod model;

pub use error::AssetError;
pub use gltf_import::{load_gltf, load_skeleton, skeleton_from_skin};
pub use library::AssetLibrary;
pub use model::{ModelData, ModelPart};
