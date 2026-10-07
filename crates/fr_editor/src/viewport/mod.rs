//! The editor's view of the preview: the camera the author looks through,
//! picking, the transform gizmo and everything drawn over the 3D frame.
//!
//! [`Viewport`] ties them together. The app shell fills a [`ViewportInput`]
//! each frame, deciding whether the pointer is over the viewport and free of
//! overlays and which keys apply, and calls [`Viewport::update`]; the viewport
//! never reads the UI. Navigation is Alt and left drag or middle drag to orbit,
//! shift and middle drag to pan, the wheel to zoom to the pivot, right drag to
//! look with W, A, S, D, Q and E to fly, and F to frame the selection. Q, W, E
//! and R choose the select, move, rotate and scale tools, X switches between
//! local and world handles, and Control snaps a drag.
//!
//! The viewport depends on `preview` and `fr_authoring`; component specific
//! shapes come from the `subsystems` registry.
//!
//! Modules:
//!
//! - `view`: the [`Viewport`]
//! - `input`: [`ViewportInput`], [`ViewportKey`] and [`ViewportOutput`]
//! - `editor_camera`: the [`EditorCamera`]
//! - `picking`: nearest-hit picking and click selection
//! - `transform_gizmo`, `handles`: the tools, handles and the drag
//! - `projector`, `grid`, `gizmo_draw`, `palette`: projecting and drawing

mod editor_camera;
mod gizmo_draw;
mod grid;
mod handles;
mod input;
mod palette;
mod picking;
mod projector;
mod transform_gizmo;
mod view;

pub use editor_camera::EditorCamera;
pub use handles::{Axis, GizmoFrame, Handle, Snap};
pub use input::{ButtonSet, CursorHint, ViewportInput, ViewportKey, ViewportOutput};
pub use palette::ViewportPalette;
pub use picking::{apply_click, pick};
pub use projector::Projector;
pub use transform_gizmo::{Space, Tool, TransformGizmo};
pub use view::Viewport;
