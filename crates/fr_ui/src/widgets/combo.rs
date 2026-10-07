//! A box that shows one choice and opens a list of the others.
//!
//! Whether the list is open is the caller's to hold. The box asks to be opened
//! by sending [`ComboEvent::Toggle`]; the list is a popup over the window, so a
//! press anywhere else sends [`ComboEvent::Dismiss`], and a row sends
//! [`ComboEvent::Select`] with its index. A list longer than a few rows scrolls,
//! when the caller gives the box a [`Scroll`] to scroll it by.

use std::sync::Arc;

use crate::div::{Div, h_flex, v_flex};
use crate::icons::{IconName, IconSize, icon};
use crate::overlay::dropdown;
use crate::style::Styled;
use crate::text::text;
use crate::theme::Theme;
use crate::widgets::{Scroll, ScrollEvent, scroll_area};

/// How many rows the list shows before it scrolls.
const VISIBLE_ROWS: usize = 10;

/// What the user did to a combo box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComboEvent {
    /// The box was clicked: open the list if it is closed, close it if open.
    Toggle,
    /// The row at this index was chosen.
    Select(usize),
    /// A press landed outside the open list: close it.
    Dismiss,
    /// The list was scrolled.
    Scroll(ScrollEvent),
}

/// A combo box showing `options[selected]`, its list open or not.
///
/// `scroll` is what a list of more than ten rows is scrolled by.
pub fn combo<M: Clone + 'static>(
    theme: &Theme,
    options: &[impl AsRef<str>],
    selected: Option<usize>,
    open: bool,
    scroll: Option<&Scroll>,
    on_event: impl Fn(ComboEvent) -> M + 'static,
) -> Div<M> {
    let on_event: Arc<dyn Fn(ComboEvent) -> M> = Arc::new(on_event);
    let label = selected
        .and_then(|index| options.get(index))
        .map(|option| option.as_ref().to_owned());
    let anchor = anchor_box(theme, label, open, on_event(ComboEvent::Toggle));
    let wrapped = v_flex().w_full();
    if !open {
        return wrapped.child(anchor);
    }
    let rows = options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let on_event = on_event.clone();
            option_row(
                theme,
                option.as_ref(),
                selected == Some(index),
                on_event(ComboEvent::Select(index)),
            )
        })
        .collect::<Vec<_>>();
    let panel = list_panel(theme, rows, options.len(), scroll, on_event.clone());
    wrapped.child(
        dropdown(anchor, panel)
            .match_width()
            .on_dismiss(on_event(ComboEvent::Dismiss)),
    )
}

/// The box that shows the current choice and opens the list.
fn anchor_box<M: Clone + 'static>(
    theme: &Theme,
    label: Option<String>,
    open: bool,
    toggle: M,
) -> Div<M> {
    let (shown, color) = match label {
        Some(label) => (label, theme.colors.text),
        None => (String::from("Select..."), theme.colors.text_subtle),
    };
    let border = if open {
        theme.colors.border_focused
    } else {
        theme.colors.border
    };
    h_flex()
        .w_full()
        .h_px(theme.size.control)
        .px(2.5)
        .gap(2)
        .items_center()
        .bg(theme.colors.surface_input)
        .hover_bg(theme.colors.surface_hover)
        .border_1(border)
        .rounded(theme.radius.md)
        .on_click(toggle)
        .child(text(shown).text_sm().color(color).flex_1())
        .child(
            icon(IconName::ChevronDown)
                .size(IconSize::Small)
                .color(theme.colors.text_muted),
        )
}

/// One choice of the list.
fn option_row<M: Clone + 'static>(theme: &Theme, label: &str, selected: bool, choose: M) -> Div<M> {
    let background = if selected {
        theme.colors.surface_selected
    } else {
        fr_color::Rgba::TRANSPARENT
    };
    h_flex()
        .w_full()
        .h_px(theme.size.control - 2.0)
        .px(2.5)
        .gap(2)
        .items_center()
        .rounded(theme.radius.sm)
        .bg(background)
        .hover_bg(theme.colors.surface_hover)
        .active_bg(theme.colors.surface_active)
        .on_click(choose)
        .child(text(label.to_owned()).text_sm().flex_1())
        .when(selected, |row| {
            row.child(
                icon(IconName::Check)
                    .size(IconSize::Small)
                    .color(theme.colors.accent),
            )
        })
}

/// The popup holding the rows, scrolled when there are many and a scroll is given.
fn list_panel<M: Clone + 'static>(
    theme: &Theme,
    rows: Vec<Div<M>>,
    count: usize,
    scroll: Option<&Scroll>,
    on_event: Arc<dyn Fn(ComboEvent) -> M>,
) -> Div<M> {
    let row_height = theme.size.control - 2.0;
    let frame = v_flex()
        .w_full()
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .blocks_pointer();
    match scroll {
        Some(scroll) if count > VISIBLE_ROWS => frame
            .h_px(row_height * VISIBLE_ROWS as f32 + 8.0)
            .p(1)
            .child(
                scroll_area(scroll, v_flex().w_full().children(rows))
                    .on_scroll(move |event| on_event(ComboEvent::Scroll(event)))
                    .h_full(),
            ),
        _ => frame.p(1).children(rows),
    }
}
