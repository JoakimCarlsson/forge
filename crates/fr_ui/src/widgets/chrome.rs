//! The chrome around panels: title bars, status bars, labelled rows and toolbars.
//!
//! These are the pieces every panel of a tool is dressed in, extracted so the
//! panels share one look. They know nothing of what a panel shows: they arrange
//! what they are given.

use fr_color::Rgba;

use crate::div::{Div, h_flex, v_flex};
use crate::element::IntoElement;
use crate::icons::{IconName, IconSize, icon};
use crate::style::{Side, Styled};
use crate::text::text;
use crate::theme::Theme;

/// The width of the label column of a property row, in logical pixels.
const LABEL_WIDTH: f32 = 104.0;

/// A bar across the top of a panel: its `title` and, at the far end, `actions`.
pub fn panel_header<M: Clone + 'static>(
    theme: &Theme,
    title: impl Into<String>,
    actions: Option<Div<M>>,
) -> Div<M> {
    h_flex()
        .w_full()
        .h_px(theme.size.control)
        .px(3)
        .gap(2)
        .items_center()
        .bg(theme.colors.surface)
        .border_side(Side::Bottom, 1.0, theme.colors.border_variant)
        .child(
            text(title)
                .text_sm()
                .font_medium()
                .color(theme.colors.text_muted)
                .flex_1(),
        )
        .when_some(actions, |bar, actions| bar.child(actions))
}

/// A bar along the bottom of a window: `start` against its left edge and `end`
/// against its right.
pub fn status_bar<M: Clone + 'static>(theme: &Theme, start: Div<M>, end: Div<M>) -> Div<M> {
    h_flex()
        .w_full()
        .h_px(theme.size.bar)
        .px(3)
        .gap(3)
        .items_center()
        .justify_between()
        .bg(theme.colors.surface)
        .border_side(Side::Top, 1.0, theme.colors.border_variant)
        .child(start)
        .child(end)
}

/// A row of a form: `label` in a fixed column and `control` taking the rest.
pub fn property_row<M: Clone + 'static>(
    theme: &Theme,
    label: impl Into<String>,
    control: impl IntoElement<M>,
) -> Div<M> {
    h_flex()
        .w_full()
        .py(0.5)
        .gap(2)
        .items_center()
        .child(
            text(label)
                .text_sm()
                .color(theme.colors.text_muted)
                .w_px(LABEL_WIDTH),
        )
        .child(v_flex().flex_1().child(control))
}

/// One button of a toolbar group: an icon that sends a message when clicked.
pub struct ToolbarButton<M> {
    /// The icon.
    icon: IconName,
    /// What clicking it sends, or nothing when it does not apply.
    message: Option<M>,
    /// Whether it is the one in use, as the current tool is.
    active: bool,
    /// What its tooltip says.
    tooltip: Option<String>,
    /// The colour of its icon, when it is not the usual one.
    tint: Option<Rgba>,
}

/// A toolbar button showing `icon`, sending `message`, greyed out without one.
pub fn toolbar_button<M>(icon: IconName, message: Option<M>) -> ToolbarButton<M> {
    ToolbarButton {
        icon,
        message,
        active: false,
        tooltip: None,
        tint: None,
    }
}

impl<M> ToolbarButton<M> {
    /// Returns this button lit as the one in use when `active`.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Returns this button showing `tooltip` once the pointer rests on it.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Returns this button's icon drawn in `color`.
    pub fn tint(mut self, color: Rgba) -> Self {
        self.tint = Some(color);
        self
    }
}

/// A group of icon buttons joined in one rounded box, as the tools of a viewport are.
pub fn toolbar_group<M: Clone + 'static>(theme: &Theme, buttons: Vec<ToolbarButton<M>>) -> Div<M> {
    h_flex()
        .p(0.5)
        .gap(0.5)
        .items_center()
        .bg(theme.colors.surface_input)
        .border_1(theme.colors.border)
        .rounded(theme.radius.md)
        .children(
            buttons
                .into_iter()
                .map(|button| toolbar_cell(theme, button)),
        )
}

/// Builds one button of a toolbar group.
fn toolbar_cell<M: Clone + 'static>(theme: &Theme, button: ToolbarButton<M>) -> Div<M> {
    let enabled = button.message.is_some();
    let color = match (enabled, button.active, button.tint) {
        (false, _, _) => theme.colors.text_subtle,
        (true, true, _) => theme.colors.accent,
        (true, false, Some(tint)) => tint,
        (true, false, None) => theme.colors.text_muted,
    };
    let background = if button.active {
        theme.colors.surface_selected
    } else {
        Rgba::TRANSPARENT
    };
    v_flex()
        .size_px(theme.size.control - 2.0)
        .items_center()
        .justify_center()
        .rounded(theme.radius.sm)
        .bg(background)
        .when(enabled, |cell| {
            cell.hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
        })
        .when_some(button.message, Div::on_click)
        .when_some(button.tooltip, Div::tooltip)
        .child(icon(button.icon).size(IconSize::Medium).color(color))
}
