//! The slider: a track and knob that picks a value in a range by press or drag.

use fr_math::{Rect, Size};
use fr_render::Quad;

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Length, Style, Styled};

/// The width of a slider that is offered no finite width.
const DEFAULT_WIDTH: f32 = 160.0;

/// The height of the area a slider answers to the pointer in.
const HEIGHT: f32 = 20.0;

/// The thickness of the track.
const TRACK_THICKNESS: f32 = 6.0;

/// The diameter of the knob.
const KNOB_DIAMETER: f32 = 14.0;

/// A track and knob that sends a message for the value under the pointer while it is pressed
/// or dragged.
pub struct Slider<M> {
    /// The value, within `min` and `max`.
    value: f32,
    /// The value at the left end.
    min: f32,
    /// The value at the right end.
    max: f32,
    /// What a press or drag sends, given the value under the pointer.
    on_change: Option<Box<dyn Fn(f32) -> M>>,
    /// How the slider is sized; it fills the offered width by default.
    style: Style,
}

/// A slider showing `value` between `min` and `max` that sends `on_change` with the value under
/// the pointer when it is pressed or dragged.
pub fn slider<M>(
    value: f32,
    min: f32,
    max: f32,
    on_change: impl Fn(f32) -> M + 'static,
) -> Slider<M> {
    Slider {
        value,
        min,
        max,
        on_change: Some(Box::new(on_change)),
        style: Style::default(),
    }
}

impl<M> Slider<M> {
    /// How far along the track the value is, from zero to one.
    fn fraction(&self) -> f32 {
        let span = self.max - self.min;
        if span > 0.0 {
            ((self.value - self.min) / span).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl<M> Styled for Slider<M> {
    /// How the slider is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for Slider<M> {
    /// How the slider is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Fills the offered width, or a default one when the offer is unbounded.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            _ if available.width.is_finite() => available.width,
            _ => DEFAULT_WIDTH,
        };
        Size::new(width, HEIGHT)
    }

    /// Registers the sliding region, then paints the track, the filled part and the knob.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let (min, max) = (self.min, self.max);
        let interaction = match self.on_change.take() {
            Some(on_change) => cx.slidable(
                bounds,
                Box::new(move |fraction| on_change(min + (max - min) * fraction)),
            ),
            None => return,
        };
        let theme = *cx.theme();
        let fraction = self.fraction();

        let travel = bounds.size.width - KNOB_DIAMETER;
        let center_y = bounds.top() + bounds.size.height * 0.5;
        let track = Rect::from_xywh(
            bounds.left() + KNOB_DIAMETER * 0.5,
            center_y - TRACK_THICKNESS * 0.5,
            travel.max(0.0),
            TRACK_THICKNESS,
        );
        cx.quad(
            Quad::filled(track, theme.colors.surface_input)
                .corner_radius(theme.radius.full)
                .border(1.0, theme.colors.border),
        );
        let filled = Rect::from_xywh(
            track.left(),
            track.top(),
            track.size.width * fraction,
            TRACK_THICKNESS,
        );
        cx.quad(Quad::filled(filled, theme.colors.accent).corner_radius(theme.radius.full));

        let knob_color = if interaction.pressed {
            theme.colors.accent_active
        } else if interaction.hovered {
            theme.colors.accent_hover
        } else {
            theme.colors.accent
        };
        let left = bounds.left() + travel.max(0.0) * fraction;
        cx.quad(
            Quad::filled(
                Rect::from_xywh(
                    left,
                    center_y - KNOB_DIAMETER * 0.5,
                    KNOB_DIAMETER,
                    KNOB_DIAMETER,
                ),
                knob_color,
            )
            .corner_radius(theme.radius.full)
            .border(1.0, theme.colors.border_focused),
        );
    }
}
