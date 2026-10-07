//! The element tree, the layout pass, hit testing, focus and input routing.
//!
//! `fr_ui` is ours: there is no UI framework under it, only [`fr_render`]'s
//! draw list. A screen is a function that builds an element tree from the
//! caller's own state; [`Ui::draw`] measures it, paints it and remembers where
//! its interactive regions ended up, so the next click or keypress comes back
//! as one of the caller's messages.
//!
//! A press that lands on a region captures the pointer until it is released, a
//! popup paints on a layer of its own over everything below it, and the generic
//! widgets (scrolling, splits, tabs, menus, text fields, number fields, trees,
//! colour fields and the dock) hand what the user did back as the caller's own
//! messages. Nothing here knows what the caller is editing.
//!
//! Styling follows Tailwind: a fixed spacing scale, one utility setter per
//! step, and design tokens on a [`Theme`] instead of colours written at the
//! call site.

mod canvas;
mod div;
mod dock;
mod drag;
mod element;
mod icons;
mod overlay;
mod rects;
mod region;
mod style;
mod text;
mod theme;
mod tooltip;
mod ui;
mod widgets;

pub use div::{Div, div, h_flex, v_flex};
pub use dock::{
    DockChange, DockDrag, DockDrop, DockEvent, DockNode, DockPanel, DockPath, DockSide,
    DockTextError, DockTree, DockView, dock_view,
};
pub use drag::{DragEvent, DragPhase};
pub use element::{Element, Input, Interaction, IntoElement, LayoutContext, PaintContext};
pub use fr_color::Rgba;
pub use fr_math::{Point, Rect, Size};
pub use icons::{Icon, IconName, IconSize, icon};
pub use overlay::{
    Above, Beside, Dropdown, Overlay, above, beside, dropdown, overlay, overlay_above, popup_at,
};
pub use rects::{Bounds, Measured, Placed, Rects, measured, placed};
pub use region::{DragHandler, PressHandler, Region, RegionAction, ScrollHandler};
pub use style::{Align, Axis, Edges, Justify, Length, STEP, Side, Style, Styled, space};
pub use text::{Text, text};
pub use theme::{Colors, Emphasis, Font, Radii, Sizes, TextScale, TextSize, Theme};
pub use ui::Ui;
pub use widgets::*;
