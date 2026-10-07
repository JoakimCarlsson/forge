//! The crate a game depends on: the [`App`] trait and the [`run`] entry point.
//!
//! Every engine crate a game needs is re-exported under a short module name:
//! [`math`], [`color`], [`time`], [`transform`], [`input`], [`light`], [`mesh`],
//! [`image`], [`material`], [`camera`], [`assets`], [`physics`],
//! [`render`] and [`ui`]. A game describes its 3D scene each frame with
//! [`App::scene`], loading meshes, materials and glTF models through [`Assets`]
//! in [`App::init`]. It steps its own [`physics::world::World`] from
//! [`App::fixed_update`], which the host calls at a fixed rate. No winit or wgpu type
//! appears in this crate's public API.

mod app;
mod host;
mod loader;

pub use app::{App, FixedStep, Frame, Input};
pub use fr_assets as assets;
pub use fr_camera as camera;
pub use fr_color as color;
pub use fr_image as image;
pub use fr_input as input;
pub use fr_light as light;
pub use fr_material as material;
pub use fr_math as math;
pub use fr_mesh as mesh;
pub use fr_physics as physics;
pub use fr_render as render;
pub use fr_time as time;
pub use fr_transform as transform;
pub use fr_ui as ui;
pub use host::{EngineError, run};
pub use loader::{Assets, LoadError};
