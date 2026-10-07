//! The dock: a layout of panels in tabs and splits, and the view that draws it.
//!
//! The layout is data ([`DockTree`], with [`DockTree::to_text`] and
//! [`DockTree::from_text`] to keep it between runs) and the view is a function
//! of it, so a dock is rebuilt every frame from the caller's own tree like
//! everything else. Neither knows what a panel shows: the caller names its
//! panels and builds each one's contents when the view asks for it.

mod text;
mod tree;
mod view;

pub use text::DockTextError;
pub use tree::{DockNode, DockPath, DockSide, DockTree};
pub use view::{DockChange, DockDrag, DockDrop, DockEvent, DockPanel, DockView, dock_view};
