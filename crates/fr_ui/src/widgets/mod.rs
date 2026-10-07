//! Widgets extracted from real screens, and only once they repeated there.
//!
//! One component to a file: a widget that paints itself is an [`Element`],
//! a widget that arranges other widgets is a function returning a [`Div`].
//!
//! [`Element`]: crate::element::Element
//! [`Div`]: crate::div::Div

mod button;
mod checkbox;
mod chrome;
mod color;
mod combo;
mod drag_value;
mod field;
mod icon_button;
mod menu;
mod rule;
mod scroll;
mod section;
mod slider;
mod split;
mod switch;
mod tabs;
mod text_edit;
mod tree;

pub use button::{Button, ButtonVariant, button};
pub use checkbox::{Checkbox, ToggleState, checkbox};
pub use chrome::{
    ToolbarButton, panel_header, property_row, status_bar, toolbar_button, toolbar_group,
};
pub use color::{
    ColorEvent, Swatch, color_editor, color_field, color_swatch, format_hex, parse_hex,
};
pub use combo::{ComboEvent, combo};
pub use drag_value::{
    DragValue, DragValueEvent, ValueRange, drag_value, format_number, parse_number, vec3_row,
};
pub use field::{Field, field, text_field};
pub use icon_button::{icon_button, tinted_icon_button};
pub use menu::{
    MenuBarEntry, MenuEntry, MenuItem, context_menu, menu, menu_bar, menu_bar_entry, menu_entry,
    menu_separator, menu_submenu,
};
pub use rule::rule;
pub use scroll::{SCROLLBAR_GUTTER, Scroll, ScrollArea, ScrollEvent, scroll_area};
pub use section::section;
pub use slider::{Slider, SliderEvent, slider};
pub use split::{Sash, Split, SplitResize, sash, split};
pub use switch::{Switch, switch};
pub use tabs::{Tab, tab, tab_bar};
pub use text_edit::{ClipboardRequest, EditOutcome, TextEdit};
pub use tree::{DropHint, TreeRow, drop_zone, list_row, tree_row};
