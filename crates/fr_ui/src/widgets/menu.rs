//! Menus of commands: the menu itself, a bar of them, and a context menu.
//!
//! A menu is a list of entries and nothing more: it has no idea what it is a
//! menu of, where it was opened from, or how it is dismissed. An entry without
//! a message is one that does not apply right now, shown greyed rather than
//! left out so a menu keeps the same shape every time it is opened. Whether a
//! menu or a submenu is open is the caller's to hold, as everything else a
//! screen shows is: the control asks to be opened by sending a message, and is
//! drawn open the next frame because the caller said so.

use fr_color::Rgba;
use fr_math::Point;

use crate::div::{Div, h_flex, v_flex};
use crate::element::{Element, IntoElement};
use crate::icons::{IconName, IconSize, icon};
use crate::overlay::{Overlay, beside, dropdown, popup_at};
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;

/// Narrowest a menu is drawn, however short its entries are.
const MIN_WIDTH: f32 = 200.0;

/// Height of the line between two groups of entries, gaps included.
const SEPARATOR_HEIGHT: f32 = 9.0;

/// The width of the column an entry's check or icon sits in.
const MARK_SLOT: f32 = 16.0;

/// One command of a menu.
pub struct MenuEntry<M> {
    /// What the entry is called.
    label: String,
    /// The key combination shown at its right edge.
    shortcut: Option<String>,
    /// The icon in its left column.
    icon: Option<IconName>,
    /// Whether it is a toggle, and whether it is on.
    checked: Option<bool>,
    /// What choosing it sends, or nothing when it does not apply.
    message: Option<M>,
}

/// One line of a menu.
pub enum MenuItem<M> {
    /// A command, which is greyed out when it carries no message.
    Entry(MenuEntry<M>),
    /// A menu of its own, opened from this line.
    Submenu {
        /// What the line is called.
        label: String,
        /// What opening it sends, or nothing when it does not apply.
        message: Option<M>,
        /// Whether it is open right now.
        expanded: bool,
        /// The lines it opens onto.
        items: Vec<MenuItem<M>>,
    },
    /// A line between two groups of commands.
    Separator,
}

/// An entry called `label` that sends `message`, greyed out without one.
pub fn menu_entry<M>(label: impl Into<String>, message: Option<M>) -> MenuItem<M> {
    MenuItem::Entry(MenuEntry {
        label: label.into(),
        shortcut: None,
        icon: None,
        checked: None,
        message,
    })
}

/// A line called `label` that opens `items` beside it when it is `expanded`.
///
/// The line asks to be opened by sending `message`; the caller draws it open
/// the next frame by passing `expanded`.
pub fn menu_submenu<M>(
    label: impl Into<String>,
    message: Option<M>,
    expanded: bool,
    items: Vec<MenuItem<M>>,
) -> MenuItem<M> {
    MenuItem::Submenu {
        label: label.into(),
        message,
        expanded,
        items,
    }
}

/// A line between two groups of entries.
pub fn menu_separator<M>() -> MenuItem<M> {
    MenuItem::Separator
}

impl<M> MenuItem<M> {
    /// Returns this entry showing `shortcut` at its right edge.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        if let Self::Entry(entry) = &mut self {
            entry.shortcut = Some(shortcut.into());
        }
        self
    }

    /// Returns this entry showing `icon` in its left column.
    pub fn icon(mut self, icon: IconName) -> Self {
        if let Self::Entry(entry) = &mut self {
            entry.icon = Some(icon);
        }
        self
    }

    /// Returns this entry a toggle that is `checked` or not.
    pub fn checked(mut self, checked: bool) -> Self {
        if let Self::Entry(entry) = &mut self {
            entry.checked = Some(checked);
        }
        self
    }
}

/// A menu of `items`, sized to its content.
pub fn menu<M: Clone + 'static>(theme: &Theme, items: Vec<MenuItem<M>>) -> Div<M> {
    v_flex()
        .p(1)
        .w_fit()
        .min_w_px(MIN_WIDTH)
        .items_stretch()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .blocks_pointer()
        .children(items.into_iter().map(|item| line(theme, item)))
}

/// A menu of `items` opened as a popup at `origin`, where a press anywhere else
/// sends `dismiss`.
pub fn context_menu<M: Clone + 'static>(
    theme: &Theme,
    origin: Point,
    dismiss: M,
    items: Vec<MenuItem<M>>,
) -> Overlay<M> {
    popup_at(origin, dismiss, menu(theme, items))
}

/// One title of a menu bar and the menu it opens.
pub struct MenuBarEntry<M> {
    /// What the title says.
    label: String,
    /// Whether the menu is open.
    open: bool,
    /// What clicking the title sends.
    toggle: M,
    /// What a press outside the open menu sends.
    dismiss: M,
    /// The lines of the menu.
    items: Vec<MenuItem<M>>,
}

/// A menu bar title called `label`, opening `items` when `open`.
///
/// Clicking the title sends `toggle`; a press anywhere outside the open menu
/// sends `dismiss`.
pub fn menu_bar_entry<M>(
    label: impl Into<String>,
    open: bool,
    toggle: M,
    dismiss: M,
    items: Vec<MenuItem<M>>,
) -> MenuBarEntry<M> {
    MenuBarEntry {
        label: label.into(),
        open,
        toggle,
        dismiss,
        items,
    }
}

/// A bar of titles along the top of a window, each opening a menu below it.
pub fn menu_bar<M: Clone + 'static>(theme: &Theme, entries: Vec<MenuBarEntry<M>>) -> Div<M> {
    h_flex()
        .w_full()
        .h_px(theme.size.bar + 3.0)
        .px(1)
        .gap(0.5)
        .items_center()
        .bg(theme.colors.surface)
        .children(entries.into_iter().map(|entry| bar_title(theme, entry)))
}

/// Builds one title of the bar, with its menu hanging from it while open.
fn bar_title<M: Clone + 'static>(theme: &Theme, entry: MenuBarEntry<M>) -> Box<dyn Element<M>> {
    let background = if entry.open {
        theme.colors.surface_active
    } else {
        Rgba::TRANSPARENT
    };
    let title = h_flex()
        .h_px(theme.size.icon_control + 4.0)
        .px(2.5)
        .items_center()
        .rounded(theme.radius.md)
        .bg(background)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(entry.toggle)
        .child(text(entry.label).text_sm().color(theme.colors.text));
    if entry.open {
        dropdown(title, menu(theme, entry.items))
            .on_dismiss(entry.dismiss)
            .into_element()
    } else {
        title.into_element()
    }
}

/// Builds one line of the menu.
fn line<M: Clone + 'static>(theme: &Theme, item: MenuItem<M>) -> Box<dyn Element<M>> {
    match item {
        MenuItem::Separator => separator(theme).into_element(),
        MenuItem::Entry(entry) => entry_row(theme, entry).into_element(),
        MenuItem::Submenu {
            label,
            message,
            expanded,
            items,
        } => {
            let row = submenu_row(theme, label, message);
            if expanded {
                beside(row, menu(theme, items)).into_element()
            } else {
                row.into_element()
            }
        }
    }
}

/// The frame every line shares: height, padding and, when it answers, the hover wash.
///
/// The label sits in a group of its own against the left edge and whatever
/// trails it against the right, with the space between them left empty rather
/// than given to the label: a line that grows would make the menu as wide as
/// the window it was offered.
fn row_frame<M: Clone + 'static>(
    theme: &Theme,
    message: Option<M>,
    leading: Div<M>,
    trailing: Option<Div<M>>,
) -> Div<M> {
    h_flex()
        .h_px(theme.size.control - 2.0)
        .px(2)
        .gap(6)
        .items_center()
        .justify_between()
        .rounded(theme.radius.sm)
        .when_some(message, |row, message| {
            row.hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(message)
        })
        .child(leading)
        .when_some(trailing, |row, trailing| row.child(trailing))
}

/// The mark and label at the left of a line.
fn leading<M: Clone + 'static>(mark: Option<IconName>, label: String, color: Rgba) -> Div<M> {
    h_flex()
        .gap(2)
        .items_center()
        .child(mark_slot(mark, color))
        .child(text(label).text_sm().color(color))
}

/// Builds a command: its mark, its label and its shortcut.
fn entry_row<M: Clone + 'static>(theme: &Theme, entry: MenuEntry<M>) -> Div<M> {
    let enabled = entry.message.is_some();
    let color = if enabled {
        theme.colors.text
    } else {
        theme.colors.text_subtle
    };
    let mark = match (entry.checked, entry.icon) {
        (Some(true), _) => Some(IconName::Check),
        (Some(false), _) => None,
        (None, name) => name,
    };
    let shortcut = entry
        .shortcut
        .map(|shortcut| h_flex().child(text(shortcut).text_xs().color(theme.colors.text_subtle)));
    row_frame(
        theme,
        entry.message,
        leading(mark, entry.label, color),
        shortcut,
    )
}

/// Builds a line that opens a menu of its own.
fn submenu_row<M: Clone + 'static>(theme: &Theme, label: String, message: Option<M>) -> Div<M> {
    let color = if message.is_some() {
        theme.colors.text
    } else {
        theme.colors.text_subtle
    };
    let arrow = h_flex().child(
        icon(IconName::ChevronRight)
            .size(IconSize::XSmall)
            .color(theme.colors.text_subtle),
    );
    row_frame(theme, message, leading(None, label, color), Some(arrow))
}

/// The fixed-width column an entry's check or icon sits in.
fn mark_slot<M: Clone + 'static>(mark: Option<IconName>, color: Rgba) -> Div<M> {
    v_flex()
        .w_px(MARK_SLOT)
        .items_center()
        .when_some(mark, |slot, name| {
            slot.child(icon(name).size(IconSize::Small).color(color))
        })
}

/// Builds the line between two groups of entries.
///
/// The line is stretched across the menu rather than told to be full width: a
/// child that asks for its parent's whole width is measured against whatever
/// the parent was offered, which for a menu is the window.
fn separator<M: Clone + 'static>(theme: &Theme) -> Div<M> {
    v_flex()
        .h_px(SEPARATOR_HEIGHT)
        .py(1)
        .w_fit()
        .items_stretch()
        .child(v_flex().h_px(1.0).bg(theme.colors.border_variant))
}
