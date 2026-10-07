//! A track with a thumb, for a value between two ends.
//!
//! Pressing the track puts the value where the pointer is and dragging moves it
//! from there. What the user does comes back as a [`SliderEvent`] so one press
//! and drag can be one gesture: [`Begin`](SliderEvent::Begin) opens it and sets
//! the value, [`Change`](SliderEvent::Change) moves it, [`Commit`](SliderEvent::Commit)
//! closes it and [`Cancel`](SliderEvent::Cancel) abandons it.

use std::sync::Arc;

use fr_color::Rgba;
use fr_math::{Point, Rect, Size};
use fr_render::Quad;

use crate::drag::{DragEvent, DragPhase};
use crate::element::{Element, LayoutContext, PaintContext};
use crate::region::RegionAction;
use crate::style::{Length, Style, Styled};

/// Height of the track.
const TRACK: f32 = 4.0;

/// Diameter of the thumb.
const THUMB: f32 = 12.0;

/// How many bands a gradient track is drawn in.
const BANDS: usize = 24;

/// What the user did to a slider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderEvent {
    /// The press landed at this value: a gesture begins.
    Begin(f64),
    /// The drag moved the value to this.
    Change(f64),
    /// The gesture ended and its last value stands.
    Commit,
    /// The gesture was abandoned: put back the value it began with.
    Cancel,
}

/// What a slider sends.
type OnEvent<M> = Arc<dyn Fn(SliderEvent) -> M>;

/// A value between two ends, drawn as a track with a thumb.
pub struct Slider<M> {
    /// The value.
    value: f64,
    /// The value at the left end.
    min: f64,
    /// The value at the right end.
    max: f64,
    /// The colours the track fades between, when it is not the plain fill.
    gradient: Option<(Rgba, Rgba)>,
    /// What the slider sends.
    on_event: OnEvent<M>,
    /// How the slider is sized.
    style: Style,
}

/// A slider showing `value` between `min` and `max`, sending `on_event`.
pub fn slider<M>(
    value: f64,
    min: f64,
    max: f64,
    on_event: impl Fn(SliderEvent) -> M + 'static,
) -> Slider<M> {
    Slider {
        value,
        min,
        max,
        gradient: None,
        on_event: Arc::new(on_event),
        style: Style {
            width: Length::Full,
            ..Style::default()
        },
    }
}

impl<M> Slider<M> {
    /// Returns this slider with a track that fades from `from` to `to`.
    pub fn gradient(mut self, from: Rgba, to: Rgba) -> Self {
        self.gradient = Some((from, to));
        self
    }

    /// The fraction of the track the value is along.
    fn fraction(&self) -> f32 {
        let span = self.max - self.min;
        if span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0) as f32
    }
}

/// The value at `x` along a track from `left` that is `width` wide.
fn value_at(min: f64, max: f64, left: f32, width: f32, x: f32) -> f64 {
    let along = ((x - left) / width.max(1.0)).clamp(0.0, 1.0);
    min + (max - min) * f64::from(along)
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

    /// Takes the width it is offered and the height of a thumb with room around it.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            _ => available.width,
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            _ => THUMB + 8.0,
        };
        Size::new(width, height)
    }

    /// Paints the track and the thumb, and takes the press and the drag.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let theme = *cx.theme();
        let inset = THUMB / 2.0;
        let left = bounds.left() + inset;
        let width = (bounds.size.width - THUMB).max(1.0);
        let (min, max) = (self.min, self.max);
        let on_press = self.on_event.clone();
        let on_drag = self.on_event.clone();
        let action = RegionAction::inert()
            .press(Arc::new(move |point: Point| {
                on_press(SliderEvent::Begin(value_at(min, max, left, width, point.x)))
            }))
            .click(self.on_event.clone()(SliderEvent::Commit))
            .drag(Arc::new(move |event: DragEvent| match event.phase {
                DragPhase::Began | DragPhase::Moved => on_drag(SliderEvent::Change(value_at(
                    min,
                    max,
                    left,
                    width,
                    event.position.x,
                ))),
                DragPhase::Ended => on_drag(SliderEvent::Commit),
                DragPhase::Cancelled => on_drag(SliderEvent::Cancel),
            }));
        let interaction = cx.region(bounds, action);

        let middle = bounds.top() + bounds.size.height / 2.0;
        let track = Rect::from_xywh(
            bounds.left(),
            middle - TRACK / 2.0,
            bounds.size.width,
            TRACK,
        );
        let reached = self.fraction();
        match self.gradient {
            Some((from, to)) => paint_gradient(cx, track, from, to),
            None => {
                cx.quad(
                    Quad::filled(track, theme.colors.surface_active).corner_radius(TRACK / 2.0),
                );
                let filled =
                    Rect::from_xywh(track.left(), track.top(), width * reached + inset, TRACK);
                cx.quad(Quad::filled(filled, theme.colors.accent).corner_radius(TRACK / 2.0));
            }
        }
        let center = Point::new(left + width * reached, middle);
        let thumb = Rect::from_xywh(center.x - inset, center.y - inset, THUMB, THUMB);
        let rim = if interaction.pressed || interaction.hovered {
            theme.colors.border_focused
        } else {
            theme.colors.border_selected
        };
        cx.quad(
            Quad::filled(thumb, theme.colors.text)
                .corner_radius(THUMB / 2.0)
                .border(1.0, rim),
        );
    }
}

/// Paints `track` as bands fading from `from` to `to`, rounded at its ends.
///
/// The bands meet on whole pixels, so no seam shows between them.
fn paint_gradient<M>(cx: &mut PaintContext<'_, '_, M>, track: Rect, from: Rgba, to: Rgba) {
    let edge =
        |index: usize| (track.left() + track.size.width * index as f32 / BANDS as f32).round();
    for index in 0..BANDS {
        let amount = index as f32 / (BANDS - 1) as f32;
        let radii = match index {
            0 => [TRACK / 2.0, 0.0, 0.0, TRACK / 2.0],
            last if last == BANDS - 1 => [0.0, TRACK / 2.0, TRACK / 2.0, 0.0],
            _ => [0.0; 4],
        };
        let (left, right) = (edge(index), edge(index + 1));
        cx.quad(
            Quad::filled(
                Rect::from_xywh(left, track.top(), right - left, track.size.height),
                from.mix(to, amount),
            )
            .corner_radii(radii),
        );
    }
}
