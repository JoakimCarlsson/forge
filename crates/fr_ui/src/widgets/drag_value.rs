//! Number fields changed by dragging sideways or by typing, and a row of three.
//!
//! A drag value shows an optional label and a number. Dragging it changes the
//! number by a step per pixel of travel: shift is a tenth of that, control ten
//! times it. A click that goes nowhere starts typing. Everything the user does
//! comes back as a [`DragValueEvent`] in the caller's own message, so one drag
//! can be one undo entry: [`Begin`](DragValueEvent::Begin) opens the gesture,
//! [`Change`](DragValueEvent::Change) carries each new value,
//! [`Commit`](DragValueEvent::Commit) closes it and
//! [`Cancel`](DragValueEvent::Cancel) abandons it, after which the caller puts
//! back the value it had at `Begin`.
//!
//! Typing is the caller's too: on [`Edit`](DragValueEvent::Edit) it starts a
//! [`TextEdit`] holding [`format_number`] of the value, feeds it keys, and
//! passes it back to the control while it lasts; on enter it reads the number
//! with [`parse_number`].

use std::sync::Arc;

use fr_color::Rgba;
use fr_math::{Point, Rect, Size};
use fr_render::Quad;

use crate::div::{Div, h_flex};
use crate::drag::{DragEvent, DragPhase};
use crate::element::{Element, LayoutContext, PaintContext};
use crate::region::RegionAction;
use crate::style::{Axis, Length, Style, Styled, space};
use crate::theme::{Font, TextSize, Theme};
use crate::widgets::TextEdit;
use crate::widgets::field::{Line, paint_line};

/// How much of the step a drag with shift held moves by.
const FINE: f64 = 0.1;

/// How many times the step a drag with control held moves by.
const COARSE: f64 = 10.0;

/// Width of the strip along the left edge of a tinted box.
const TINT_STRIP: f32 = 3.0;

/// What the user did to a drag value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragValueEvent {
    /// A drag began: what follows is one gesture.
    Begin,
    /// The value is now this, within the range and rounded to the precision.
    Change(f64),
    /// The drag ended and its last value stands.
    Commit,
    /// The drag was abandoned: put back the value it began with.
    Cancel,
    /// The value was clicked without dragging: start typing it.
    Edit,
    /// The pointer went down in the text being typed, or dragged there, at this
    /// character; the flag says whether to extend the selection.
    Caret(usize, bool),
}

/// The values a number may take; either end may be open.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ValueRange {
    /// The smallest value.
    pub min: Option<f64>,
    /// The largest value.
    pub max: Option<f64>,
}

impl ValueRange {
    /// Any value.
    pub const NONE: Self = Self {
        min: None,
        max: None,
    };

    /// Values from `low` to `high`.
    pub const fn between(low: f64, high: f64) -> Self {
        Self {
            min: Some(low),
            max: Some(high),
        }
    }

    /// `value` held inside the range.
    pub fn clamp(self, value: f64) -> f64 {
        let capped = self.max.map_or(value, |max| value.min(max));
        self.min.map_or(capped, |min| capped.max(min))
    }
}

/// `value` written with `precision` decimals, as a drag value shows it.
pub fn format_number(value: f64, precision: usize) -> String {
    format!("{value:.precision$}")
}

/// The number written in `text`, accepting a comma as the decimal mark.
pub fn parse_number(text: &str) -> Option<f64> {
    let value: f64 = text.trim().replace(',', ".").parse().ok()?;
    value.is_finite().then_some(value)
}

/// What a drag value sends.
type OnEvent<M> = Arc<dyn Fn(DragValueEvent) -> M>;

/// A number shown in a box, changed by dragging it or typing over it.
pub struct DragValue<M> {
    /// The text before the number, when there is any.
    label: Option<String>,
    /// The colour of the label and of the strip along the box, when tinted.
    tint: Option<Rgba>,
    /// The number.
    value: f64,
    /// The values it may take.
    range: ValueRange,
    /// How much the value changes per pixel of travel.
    step: f64,
    /// How many decimals are shown and kept.
    precision: usize,
    /// The text after the number, when there is any.
    suffix: Option<String>,
    /// The text being typed, while the box is a field.
    edit: Option<TextEdit>,
    /// What the control sends.
    on_event: OnEvent<M>,
    /// How the box is sized.
    style: Style,
}

/// A number box showing `value`, sending `on_event` for what the user does to it.
pub fn drag_value<M>(value: f64, on_event: impl Fn(DragValueEvent) -> M + 'static) -> DragValue<M> {
    DragValue {
        label: None,
        tint: None,
        value,
        range: ValueRange::NONE,
        step: 0.1,
        precision: 2,
        suffix: None,
        edit: None,
        on_event: Arc::new(on_event),
        style: Style {
            width: Length::Full,
            ..Style::default()
        },
    }
}

impl<M> DragValue<M> {
    /// Returns this box labelled `label`.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Returns this box tinted `color`: its label drawn in it and a strip along its edge.
    pub fn tint(mut self, color: Rgba) -> Self {
        self.tint = Some(color);
        self
    }

    /// Returns this box keeping the value between `min` and `max`.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.range = ValueRange::between(min, max);
        self
    }

    /// Returns this box changing the value by `step` per pixel of drag.
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Returns this box showing and keeping `precision` decimals.
    pub fn precision(mut self, precision: usize) -> Self {
        self.precision = precision;
        self
    }

    /// Returns this box for whole numbers: no decimals, a step of one per pixel.
    pub fn integer(mut self) -> Self {
        self.precision = 0;
        self.step = 1.0;
        self
    }

    /// Returns this box showing `suffix` after the number, a unit for instance.
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// Returns this box as a text field showing `edit`, while one is being typed.
    pub fn editing(mut self, edit: Option<&TextEdit>) -> Self {
        self.edit = edit.cloned();
        self
    }

    /// The handler of a drag that begins now, bound to the value it begins from.
    fn drag_handler(&self) -> crate::region::DragHandler<M>
    where
        M: 'static,
    {
        let on_event = self.on_event.clone();
        let (start, range, step, precision) = (self.value, self.range, self.step, self.precision);
        Arc::new(move |event: DragEvent| match event.phase {
            DragPhase::Began => on_event(DragValueEvent::Begin),
            DragPhase::Moved => on_event(DragValueEvent::Change(dragged_value(
                start, range, step, precision, event,
            ))),
            DragPhase::Ended => on_event(DragValueEvent::Commit),
            DragPhase::Cancelled => on_event(DragValueEvent::Cancel),
        })
    }

    /// The text of the value, with its suffix.
    fn shown(&self) -> String {
        let number = format_number(self.value, self.precision);
        match &self.suffix {
            Some(suffix) => format!("{number} {suffix}"),
            None => number,
        }
    }
}

/// The value a drag that began at `start` has reached at `event`.
fn dragged_value(
    start: f64,
    range: ValueRange,
    step: f64,
    precision: usize,
    event: DragEvent,
) -> f64 {
    let factor = if event.modifiers.shift {
        FINE
    } else if event.modifiers.control {
        COARSE
    } else {
        1.0
    };
    let travelled = f64::from(event.total_along(Axis::Horizontal));
    let scale = 10f64.powi(precision as i32);
    let raw = start + travelled * step * factor;
    range.clamp((raw * scale).round() / scale)
}

impl<M> Styled for DragValue<M> {
    /// How the box is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for DragValue<M> {
    /// How the box is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes the width it is offered and the height of a control.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => available.width,
            Length::Auto => space(24.0),
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            _ => cx.theme.size.control,
        };
        Size::new(width, height)
    }

    /// Paints the box, its label and number or its text field, and takes the drag.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let theme = *cx.theme();
        let editing = self.edit.is_some();
        let interaction = self.register(bounds, cx);
        let border = match (editing, interaction.pressed, interaction.hovered) {
            (true, _, _) | (_, true, _) => theme.colors.border_focused,
            (_, _, true) => theme.colors.border_selected,
            _ => theme.colors.border,
        };
        cx.quad(
            Quad::filled(bounds, theme.colors.surface_input)
                .corner_radius(theme.radius.md)
                .border(1.0, border),
        );
        if let Some(tint) = self.tint {
            cx.quad(
                Quad::filled(
                    Rect::from_xywh(bounds.left(), bounds.top(), TINT_STRIP, bounds.size.height),
                    tint,
                )
                .corner_radii([theme.radius.md, 0.0, 0.0, theme.radius.md]),
            );
        }
        let pad = space(2.0);
        let inner = Rect::from_xywh(
            bounds.left() + pad + if self.tint.is_some() { TINT_STRIP } else { 0.0 },
            bounds.top(),
            (bounds.size.width - pad * 2.0 - if self.tint.is_some() { TINT_STRIP } else { 0.0 })
                .max(0.0),
            bounds.size.height,
        );
        let font = Font::new(TextSize::Sm).resolve(&theme.text);
        let label_width = self.paint_label(inner, cx);
        let value_area = Rect::from_xywh(
            inner.left() + label_width,
            inner.top(),
            (inner.size.width - label_width).max(0.0),
            inner.size.height,
        );
        match &self.edit {
            Some(edit) => paint_line(
                cx,
                value_area,
                font,
                &Line {
                    value: edit.text(),
                    placeholder: "",
                    caret: edit.caret(),
                    selection: edit.selection(),
                    focused: true,
                    color: theme.colors.text,
                },
            ),
            None => {
                let shown = self.shown();
                let run = cx.shape(&shown, font);
                let origin = Point::new(
                    (value_area.right() - run.width).round(),
                    inner.top() + ((inner.size.height - run.height) / 2.0).round(),
                );
                cx.text(origin, run, theme.colors.text);
            }
        }
    }
}

impl<M: 'static> DragValue<M> {
    /// Registers the region that takes the drag, or places the caret while typing.
    fn register(
        &self,
        bounds: Rect,
        cx: &mut PaintContext<'_, '_, M>,
    ) -> crate::element::Interaction {
        let on_event = self.on_event.clone();
        if self.edit.is_none() {
            let action = RegionAction::inert()
                .click(on_event(DragValueEvent::Edit))
                .drag(self.drag_handler());
            return cx.region(bounds, action);
        }
        let press = on_event.clone();
        let drag = on_event;
        let left = bounds.left();
        let offsets_font = Font::new(TextSize::Sm).resolve(&cx.theme().text);
        let text_value = self
            .edit
            .as_ref()
            .map(|edit| edit.text().to_owned())
            .unwrap_or_default();
        let offsets = crate::widgets::field::caret_offsets(cx, &text_value, offsets_font);
        let origin = left + space(2.0) + self.label_extent(cx);
        let offsets_drag = offsets.clone();
        let action = RegionAction::inert()
            .press(Arc::new(move |point: Point| {
                press(DragValueEvent::Caret(
                    crate::widgets::field::index_at(&offsets, point.x - origin),
                    false,
                ))
            }))
            .drag(Arc::new(move |event: DragEvent| {
                drag(DragValueEvent::Caret(
                    crate::widgets::field::index_at(&offsets_drag, event.position.x - origin),
                    true,
                ))
            }));
        cx.region(bounds, action)
    }

    /// The width the label takes before the value, gap included.
    fn label_extent(&self, cx: &mut PaintContext<'_, '_, M>) -> f32 {
        let Some(label) = &self.label else {
            return if self.tint.is_some() { TINT_STRIP } else { 0.0 };
        };
        let font = Font::new(TextSize::Xs).resolve(&cx.theme().text);
        let strip = if self.tint.is_some() { TINT_STRIP } else { 0.0 };
        strip + cx.measure(label, font).width + space(2.0)
    }

    /// Paints the label at the left of `inner`, returning the width it took.
    fn paint_label(&self, inner: Rect, cx: &mut PaintContext<'_, '_, M>) -> f32 {
        let Some(label) = &self.label else {
            return 0.0;
        };
        let theme = *cx.theme();
        let font = Font::new(TextSize::Xs).resolve(&theme.text);
        let run = cx.shape(label, font);
        let color = self.tint.unwrap_or(theme.colors.text_muted);
        let origin = Point::new(
            inner.left(),
            inner.top() + ((inner.size.height - run.height) / 2.0).round(),
        );
        let width = run.width + space(2.0);
        cx.text(origin, run, color);
        width
    }
}

/// A row of three drag values for X, Y and Z, each tagged in its axis colour.
///
/// `on_event` is told which axis, `0` to `2`, an event belongs to. `editing`
/// names the axis being typed and the text being typed into it.
pub fn vec3_row<M: 'static>(
    theme: &Theme,
    values: [f64; 3],
    step: f64,
    editing: Option<(usize, &TextEdit)>,
    on_event: impl Fn(usize, DragValueEvent) -> M + Clone + 'static,
) -> Div<M> {
    let tags = [
        ("X", theme.colors.axis_x),
        ("Y", theme.colors.axis_y),
        ("Z", theme.colors.axis_z),
    ];
    h_flex()
        .w_full()
        .gap(1)
        .children(tags.into_iter().enumerate().map(|(axis, (label, color))| {
            let on_event = on_event.clone();
            let edit = editing.and_then(|(at, edit)| (at == axis).then_some(edit));
            drag_value(values[axis], move |event| on_event(axis, event))
                .label(label)
                .tint(color)
                .step(step)
                .editing(edit)
                .flex_1()
        }))
}
