//! CPU-side mesh data and the built-in primitive shapes of the forge engine.
//!
//! Nothing here touches a graphics device: a [`MeshData`] is plain vectors of
//! vertex attributes that a renderer uploads and names with a [`MeshId`]. A [`Skin`] binds a
//! mesh to joints of an `fr_transform` hierarchy.
//!
//! The crate root is the facade and nothing else.

mod mesh;
mod primitives;
mod skin;

pub use mesh::{MeshData, MeshId};
pub use primitives::{capsule, cube, cylinder, plane, sphere};
pub use skin::{Skin, SkinError};
