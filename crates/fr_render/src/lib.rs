//! The wgpu device, glyph atlas, text shaping and draw-list submission of the
//! forge engine.
//!
//! Callers build a [`DrawList`] of quads, shaped text and SVG icons in logical pixels, and
//! optionally a 3D [`Scene`] of lit [`MeshInstance`]s, and hand both to
//! [`Renderer::render`], which draws the scene and the UI over it in one
//! submission. Meshes, materials and textures are uploaded once and named by the
//! `MeshId`, `MaterialId` and `TextureId` handles of `fr_mesh`, `fr_material` and
//! `fr_image`. [`Renderer::render_in`] confines the 3D pass to a sub-rectangle of
//! the frame, and [`Renderer::new_offscreen`] with [`Renderer::capture`] renders
//! without a window and reads the frame back as a [`Capture`]. Queues, encoders, pipelines and bind groups stay behind this boundary.
//!
//! The crate root is the facade and nothing else: every type lives in the
//! module that owns it, and this file only says which ones callers may name.

mod atlas;
mod capture;
mod draw;
mod error;
mod forward;
mod mipmaps;
mod pipeline;
mod renderer;
mod resources;
mod scene;
mod shadows;
mod svg;
mod targets;
mod text;
mod uniforms;

pub use capture::Capture;
pub use draw::{DrawList, IconRun, Quad, TextRun};
pub use error::RenderError;
pub use renderer::Renderer;
pub use scene::{AmbientLight, MeshInstance, Model, Scene};
pub use svg::Svg;
pub use text::{FontFamily, FontStyle, ShapedRun, TextSystem};
