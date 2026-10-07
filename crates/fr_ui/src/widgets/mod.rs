//! Widgets extracted from real screens, and only once they repeated there.
//!
//! One component to a file: a widget that paints itself is an [`Element`],
//! a widget that arranges other widgets is a function returning a [`Div`].
//!
//! [`Element`]: crate::element::Element
//! [`Div`]: crate::div::Div

mod button;
mod checkbox;
mod icon_button;
mod rule;
mod section;
mod switch;

pub use button::{Button, ButtonVariant, button};
pub use checkbox::{Checkbox, ToggleState, checkbox};
pub use icon_button::{icon_button, tinted_icon_button};
pub use rule::rule;
pub use section::section;
pub use switch::{Switch, switch};
