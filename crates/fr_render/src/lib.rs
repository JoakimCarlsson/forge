//! The wgpu device, glyph atlas, text shaping and draw-list submission of the
//! forge engine.
//!
//! Callers build a [`DrawList`] of quads, shaped text and SVG icons in logical pixels, and
//! optionally a 3D [`Scene`] of lit [`MeshInstance`]s, and hand both to
//! [`Renderer::render`], which draws the scene and the UI over it in one
//! submission. Meshes, materials and textures are uploaded once and named by the
//! [`MeshId`], [`MaterialId`] and [`TextureId`] handles of `fr_core`. Queues,
//! encoders, pipelines and bind groups stay behind this boundary.
//!
//! The crate root is the facade and nothing else: every type lives in the
//! module that owns it, and this file only says which ones callers may name.

mod atlas;
mod color;
mod draw;
mod error;
mod forward;
mod geometry;
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

pub use color::Rgba;
pub use draw::{DrawList, IconRun, Quad, TextRun};
pub use error::RenderError;
pub use geometry::{Point, Rect, Size};
pub use renderer::Renderer;
pub use scene::{AmbientLight, Camera, MeshInstance, Model, Scene};
pub use svg::Svg;
pub use text::{FontFamily, FontStyle, ShapedRun, TextSystem};
