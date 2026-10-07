//! A box that is ticked, empty, or neither.

use fr_color::Rgba;
use fr_math::{Rect, Size};
use fr_render::Quad;

use crate::element::{Element, Interaction, LayoutContext, PaintContext};
use crate::icons::{IconName, IconSize};
use crate::style::{Style, Styled};
use crate::theme::Colors;

/// The side of the square the box is drawn at.
const SIDE: f32 = 16.0;

/// Space between the box and the dash that marks a mixed box on each side.
const MARK_INSET: f32 = 4.0;

/// Height of the bar that marks a box that is neither ticked nor empty.
const MIXED_BAR: f32 = 2.0;

/// What a box that can be ticked is saying.
///
/// The third state is what a box says about several things at once when they
/// do not agree: some of this file is staged and some of it is not, and a box
/// that showed either of the other two states would be wrong about half of it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToggleState {
    /// None of it.
    #[default]
    Off,
    /// Some of it, but not all.
    Mixed,
    /// All of it.
    On,
}

impl ToggleState {
    /// Whether the box is filled at all.
    fn is_filled(self) -> bool {
        self != Self::Off
    }
}

/// A box in `state` that sends `message` when it is clicked.
pub struct Checkbox<M> {
    /// What the box is saying.
    state: ToggleState,
    /// What a click sends.
    message: M,
    /// How the box is sized; it has a fixed size by default.
    style: Style,
}

/// A box in `state` that sends `message` when it is clicked.
pub fn checkbox<M>(state: ToggleState, message: M) -> Checkbox<M> {
    Checkbox {
        state,
        message,
        style: Style::default(),
    }
}

impl<M> Styled for Checkbox<M> {
    /// How the box is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: Clone> Element<M> for Checkbox<M> {
    /// How the box is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Reports the fixed side of the square.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        Size::new(SIDE, SIDE)
    }

    /// Registers the click target, then paints the box and its mark.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let interaction = cx.interactive(bounds, self.message.clone());
        let theme = *cx.theme();

        cx.quad(
            Quad::filled(bounds, self.fill(&theme.colors, interaction))
                .corner_radius(theme.radius.sm)
                .border(1.0, self.outline(&theme.colors, interaction)),
        );

        match self.state {
            ToggleState::Off => {}
            ToggleState::Mixed => {
                let dash = Rect::from_xywh(
                    bounds.left() + MARK_INSET,
                    bounds.top() + (bounds.size.height - MIXED_BAR) / 2.0,
                    bounds.size.width - MARK_INSET * 2.0,
                    MIXED_BAR,
                );
                cx.quad(
                    Quad::filled(dash, theme.colors.text_on_accent).corner_radius(theme.radius.sm),
                );
            }
            ToggleState::On => {
                let side = IconSize::XSmall.pixels();
                let tick = Rect::from_xywh(
                    (bounds.left() + (bounds.size.width - side) / 2.0).round(),
                    (bounds.top() + (bounds.size.height - side) / 2.0).round(),
                    side,
                    side,
                );
                cx.rotated_icon(
                    tick,
                    IconName::Check.svg(),
                    theme.colors.text_on_accent,
                    0.0,
                );
            }
        }
    }
}

impl<M> Checkbox<M> {
    /// The fill of the box in `interaction`.
    fn fill(&self, colors: &Colors, interaction: Interaction) -> Rgba {
        match (self.state.is_filled(), interaction.hovered) {
            (true, true) => colors.accent_hover,
            (true, false) => colors.accent,
            (false, true) => colors.surface_hover,
            (false, false) => colors.surface_input,
        }
    }

    /// The outline of the box in `interaction`.
    fn outline(&self, colors: &Colors, interaction: Interaction) -> Rgba {
        match (interaction.focused, self.state.is_filled()) {
            (true, _) => colors.border_focused,
            (false, true) => colors.accent,
            (false, false) => colors.border_selected,
        }
    }
}
