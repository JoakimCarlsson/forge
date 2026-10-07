//! The element tree, the layout pass, hit testing, focus and input routing.
//!
//! `fr_ui` is ours: there is no UI framework under it, only [`fr_render`]'s
//! draw list. A screen is a function that builds an element tree from the
//! caller's own state; [`Ui::draw`] measures it, paints it and remembers where
//! its interactive regions ended up, so the next click or keypress comes back
//! as one of the caller's messages.
//!
//! Styling follows Tailwind: a fixed spacing scale, one utility setter per
//! step, and design tokens on a [`Theme`] instead of colours written at the
//! call site.

mod div;
mod element;
mod icons;
mod style;
mod text;
mod theme;
mod ui;
mod widgets;

pub use div::{Div, div, h_flex, v_flex};
pub use element::{
    Element, Input, Interaction, IntoElement, LayoutContext, PaintContext, Region, RegionAction,
};
pub use fr_render::{Point, Rect, Rgba, Size};
pub use icons::{Icon, IconName, IconSize, icon};
pub use style::{Align, Axis, Edges, Justify, Length, STEP, Side, Style, Styled, space};
pub use text::{Text, text};
pub use theme::{Colors, Emphasis, Font, Radii, Sizes, TextScale, TextSize, Theme};
pub use ui::Ui;
pub use widgets::{
    Button, ButtonVariant, Checkbox, Switch, ToggleState, button, checkbox, icon_button, rule,
    section, switch, tinted_icon_button,
};
