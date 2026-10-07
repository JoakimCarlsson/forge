//! The crate a game depends on: the [`App`] trait and the [`run`] entry point.
//!
//! The UI vocabulary a game builds its tree from is re-exported as [`ui`]. A
//! game describes its 3D scene each frame with [`App::scene`], loading meshes,
//! materials and glTF models through [`Assets`] in [`App::init`]; the CPU-side
//! asset types are re-exported as [`assets`]. No winit or wgpu type appears in
//! this crate's public API.

mod app;
mod host;
mod loader;

pub use app::{App, Frame, Input};
pub use fr_assets as assets;
pub use fr_core::{
    DirectionalLight, Light, Mat4, MaterialId, MeshId, PointLight, Quat, SpotLight, TextureId,
    Transform, Vec2, Vec3, Vec4,
};
pub use fr_render::{AmbientLight, Camera, MeshInstance, Model, Scene};
pub use fr_ui as ui;
pub use fr_window::{ButtonState, Key, KeyEvent, Modifiers, PointerButton, ScrollDelta};
pub use host::{EngineError, run};
pub use loader::{Assets, LoadError};
