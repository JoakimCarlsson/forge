//! The bar of tabs above a pane, and one tab in it.
//!
//! Every pane wears the same bar: whatever is open in it, one tab each, the one
//! in front lit, and the pane's own controls at the end of the row. The bar
//! knows nothing of what the tabs hold, only what each one is called, what
//! clicking it sends and, for a tab that can be carried to another bar, the
//! drag events it sends. A press that went nowhere is a click and selects the
//! tab; one that travelled is a drag, and ends wherever the caller finds the
//! pointer, which a [`Rects`] recorded by the tabs and the bars can tell it.

use std::sync::Arc;

use fr_color::Rgba;

use crate::div::{Div, h_flex, v_flex};
use crate::drag::DragEvent;
use crate::icons::{IconName, IconSize, icon};
use crate::rects::Rects;
use crate::style::{Side, Styled};
use crate::text::text;
use crate::theme::Theme;
use crate::widgets::icon_button;

/// Longest title a tab shows before it is cut short.
const TITLE_CHARS: usize = 24;

/// Diameter of the dot marking that what a tab holds is unsaved.
const DOT_SIZE: f32 = 7.0;

/// Thickness of the line across the top of the tab in front of the pane that
/// has the keyboard.
const LIT_EDGE: f32 = 2.0;

/// What carrying one tab across the window sends, event by event.
type OnDrag<M> = Arc<dyn Fn(DragEvent) -> M>;

/// One tab in a pane's bar of them.
pub struct Tab<M> {
    /// What the tab shows before its title.
    icon: Option<IconName>,
    /// What the tab calls what is in it.
    title: String,
    /// Whether this is the one the pane is showing.
    active: bool,
    /// Whether what is in it has changes that are not saved.
    dirty: bool,
    /// What clicking the tab sends.
    select: M,
    /// What closing the tab sends, when it can be closed.
    close: Option<M>,
    /// What clicking it with the secondary button sends.
    secondary: Option<M>,
    /// What carrying it sends, when it can be carried at all.
    drag: Option<OnDrag<M>>,
    /// Where to leave the tab's bounds, and under which key.
    record: Option<(Rects, String)>,
}

/// A tab called `title` that sends `select` when it is clicked.
pub fn tab<M>(title: impl Into<String>, select: M) -> Tab<M> {
    Tab {
        icon: None,
        title: title.into(),
        active: false,
        dirty: false,
        select,
        close: None,
        secondary: None,
        drag: None,
        record: None,
    }
}

impl<M> Tab<M> {
    /// Returns this tab showing `icon` before its title.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Returns this tab shown as the one its pane is in front of when `active`.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Returns this tab marked as holding unsaved changes when `dirty`.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// Returns this tab with a close control sending `message`.
    pub fn on_close(mut self, message: M) -> Self {
        self.close = Some(message);
        self
    }

    /// Returns this tab sending `message` when the secondary button goes down on it.
    pub fn on_secondary(mut self, message: M) -> Self {
        self.secondary = Some(message);
        self
    }

    /// Returns this tab carried by the pointer, reporting through `on_drag`.
    pub fn on_drag(mut self, on_drag: impl Fn(DragEvent) -> M + 'static) -> Self {
        self.drag = Some(Arc::new(on_drag));
        self
    }

    /// Returns this tab leaving its bounds in `rects` under `key` as it paints.
    pub fn recorded(mut self, rects: &Rects, key: impl Into<String>) -> Self {
        self.record = Some((rects.clone(), key.into()));
        self
    }
}

/// Builds a pane's bar: one tab per thing open in it, then its own actions.
///
/// The empty stretch after the last tab is part of the bar rather than a gap in
/// it: a tab let go of there belongs at the end of the row. The hairline under
/// the bar is drawn by each part of it rather than across it, so the tab in
/// front leaves a gap in it and reads as one piece with what is drawn beneath.
/// The tab in front of a `focused` pane is also lit along its top edge, which
/// is how the window says where keystrokes go.
pub fn tab_bar<M: Clone + 'static>(
    theme: &Theme,
    tabs: Vec<Tab<M>>,
    actions: Option<Div<M>>,
    focused: bool,
) -> Div<M> {
    let floor = theme.colors.border_variant;
    h_flex()
        .w_full()
        .h_px(theme.size.tab_bar)
        .items_stretch()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .children(tabs.into_iter().map(|tab| one_tab(theme, tab, focused)))
        .child(h_flex().flex_1().border_side(Side::Bottom, 1.0, floor))
        .when_some(actions, |bar, actions| {
            bar.child(actions.border_side(Side::Bottom, 1.0, floor))
        })
}

/// Builds one tab: its edges, what it holds and the control that closes it.
fn one_tab<M: Clone + 'static>(theme: &Theme, tab: Tab<M>, focused: bool) -> Div<M> {
    let (background, color) = if tab.active {
        (theme.colors.background, theme.colors.text)
    } else {
        (theme.colors.surface, theme.colors.text_muted)
    };
    let title = text(truncated(&tab.title, TITLE_CHARS))
        .text_sm()
        .color(color);

    h_flex()
        .h_full()
        .pl(2.5)
        .pr(1)
        .gap(1.5)
        .items_center()
        .overflow_hidden()
        .bg(background)
        .border_side(Side::Right, 1.0, theme.colors.border_variant)
        .when(tab.active && focused, |row| {
            row.border_side(Side::Top, LIT_EDGE, theme.colors.border_focused)
        })
        .when(!tab.active, |row| {
            row.hover_bg(theme.colors.surface_hover).border_side(
                Side::Bottom,
                1.0,
                theme.colors.border_variant,
            )
        })
        .when_some(tab.drag, |row, on_drag| {
            row.on_drag(move |event| on_drag(event))
        })
        .when_some(tab.secondary, Div::on_secondary_click)
        .when_some(tab.record, |row, (rects, key)| row.recorded(&rects, key))
        .on_click(tab.select)
        .when_some(tab.icon, |row, name| {
            row.child(
                icon(name)
                    .size(IconSize::Medium)
                    .color(theme.colors.text_subtle),
            )
        })
        .child(title)
        .when(tab.dirty, |row| row.child(unsaved_dot(theme, color)))
        .when_some(tab.close, |row, close| {
            row.child(icon_button(theme, IconName::Close, close))
        })
}

/// Builds the mark a tab carries while what it holds is not saved.
///
/// The mark is drawn in the tab's own text colour: it says something about the
/// title beside it, and it dims with that title when the tab is not the one in
/// front.
fn unsaved_dot<M>(theme: &Theme, color: Rgba) -> Div<M> {
    v_flex()
        .size_px(DOT_SIZE)
        .rounded(theme.radius.full)
        .bg(color)
}

/// `title` cut to `chars` characters, ending in an ellipsis when it was cut.
fn truncated(title: &str, chars: usize) -> String {
    if title.chars().count() <= chars {
        return title.to_owned();
    }
    title
        .chars()
        .take(chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}
