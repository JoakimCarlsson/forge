//! Colour swatches and a small colour editor that opens from one.
//!
//! A colour field is a swatch and its hex code in a box. Clicking it opens an
//! editor as a popup: a preview, a slider for each of red, green, blue and
//! alpha, and the hex code, which can be typed over. Whether the popup is open
//! and what is being typed are the caller's. Edits come back as
//! [`ColorEvent`]s shaped like a drag value's: [`Begin`](ColorEvent::Begin) and
//! [`Change`](ColorEvent::Change) carry the live colour, and
//! [`Commit`](ColorEvent::Commit) or [`Cancel`](ColorEvent::Cancel) close the
//! gesture, so one drag can be one undo entry.

use std::sync::Arc;

use fr_color::Rgba;
use fr_math::{Point, Rect, Size};
use fr_render::Quad;

use crate::div::{Div, h_flex, v_flex};
use crate::drag::DragEvent;
use crate::element::{Element, LayoutContext, PaintContext};
use crate::icons::{IconName, IconSize, icon};
use crate::overlay::dropdown;
use crate::region::RegionAction;
use crate::style::{Length, Style, Styled};
use crate::text::text;
use crate::theme::Theme;
use crate::widgets::field::{Line, caret_offsets, index_at, paint_line};
use crate::widgets::{SliderEvent, TextEdit, slider};

/// Width of the editor popup.
const EDITOR_WIDTH: f32 = 240.0;

/// The side of the swatch square inside a colour field.
const SWATCH: f32 = 16.0;

/// What the user did to a colour field or its editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorEvent {
    /// The field was clicked: open the editor if closed, close it if open.
    Toggle,
    /// A press landed outside the open editor: close it.
    Dismiss,
    /// A gesture begins, with the colour it sets.
    Begin(Rgba),
    /// The colour is now this.
    Change(Rgba),
    /// The gesture ended and its last colour stands.
    Commit,
    /// The gesture was abandoned: put back the colour it began with.
    Cancel,
    /// The hex code was clicked: start typing it.
    HexEdit,
    /// The pointer went down in the hex code being typed, or dragged there, at
    /// this character; the flag says whether to extend the selection.
    HexCaret(usize, bool),
}

/// `color` as a hex code: six digits, or eight when it is not opaque.
pub fn format_hex(color: Rgba) -> String {
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b, a) = (byte(color.r), byte(color.g), byte(color.b), byte(color.a));
    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

/// The colour written as the hex code `text`, with three, four, six or eight
/// digits and an optional leading `#`.
pub fn parse_hex(text: &str) -> Option<Rgba> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let expanded: String = match digits.len() {
        3 | 4 => digits.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => digits.to_owned(),
        _ => return None,
    };
    let value = u32::from_str_radix(&expanded, 16).ok()?;
    Some(if expanded.len() == 6 {
        Rgba::hex(value)
    } else {
        Rgba::hexa(value)
    })
}

/// `color` with channel `index`, `0` to `3` for red to alpha, set to `value`.
fn with_channel(color: Rgba, index: usize, value: f32) -> Rgba {
    let value = value.clamp(0.0, 1.0);
    match index {
        0 => Rgba { r: value, ..color },
        1 => Rgba { g: value, ..color },
        2 => Rgba { b: value, ..color },
        _ => Rgba { a: value, ..color },
    }
}

/// A rounded square filled with `color`, over a pair of greys when it is not opaque.
pub struct Swatch {
    /// What the square shows.
    color: Rgba,
    /// How the square is sized.
    style: Style,
}

impl<M> Element<M> for Swatch {
    /// How the square is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// The width and height of the style, or the swatch side.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => available.width,
            Length::Auto => SWATCH,
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            _ => SWATCH,
        };
        Size::new(width, height)
    }

    /// Paints a chequer behind the colour, then the colour, then a hairline.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let theme = *cx.theme();
        let radius = theme.radius.sm;
        let half = bounds.size.width / 2.0;
        let rows = bounds.size.height / 2.0;
        if self.color.a < 1.0 {
            let dark = theme.colors.text_subtle;
            let light = theme.colors.text_muted;
            cx.quad(Quad::filled(bounds, light).corner_radius(radius));
            cx.quad(Quad::filled(
                Rect::from_xywh(bounds.left(), bounds.top(), half, rows),
                dark,
            ));
            cx.quad(Quad::filled(
                Rect::from_xywh(bounds.left() + half, bounds.top() + rows, half, rows),
                dark,
            ));
        }
        cx.quad(
            Quad::filled(bounds, self.color)
                .corner_radius(radius)
                .border(1.0, theme.colors.border_selected),
        );
    }
}

/// A square of `color`, sixteen pixels a side unless sized otherwise.
pub fn color_swatch(color: Rgba) -> Swatch {
    Swatch {
        color,
        style: Style::default(),
    }
}

impl Styled for Swatch {
    /// How the square is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

/// A box showing a swatch of `color` and its hex code, opening the editor when clicked.
///
/// `hex` is the code being typed, when the caller has one.
pub fn color_field<M: Clone + 'static>(
    theme: &Theme,
    color: Rgba,
    open: bool,
    hex: Option<&TextEdit>,
    on_event: impl Fn(ColorEvent) -> M + 'static,
) -> Div<M> {
    let on_event: Arc<dyn Fn(ColorEvent) -> M> = Arc::new(on_event);
    let border = if open {
        theme.colors.border_focused
    } else {
        theme.colors.border
    };
    let anchor = h_flex()
        .w_full()
        .h_px(theme.size.control)
        .px(2.5)
        .gap(2)
        .items_center()
        .bg(theme.colors.surface_input)
        .hover_bg(theme.colors.surface_hover)
        .border_1(border)
        .rounded(theme.radius.md)
        .on_click(on_event(ColorEvent::Toggle))
        .child(color_swatch(color).size_px(SWATCH))
        .child(text(format_hex(color)).text_sm().flex_1())
        .child(
            icon(IconName::ChevronDown)
                .size(IconSize::Small)
                .color(theme.colors.text_muted),
        );
    let wrapped = v_flex().w_full();
    if !open {
        return wrapped.child(anchor);
    }
    let editor_events = on_event.clone();
    wrapped.child(
        dropdown(
            anchor,
            color_editor(theme, color, hex, move |event| editor_events(event)),
        )
        .on_dismiss(on_event(ColorEvent::Dismiss)),
    )
}

/// The editor itself: a preview, four sliders and the hex code.
///
/// Used by [`color_field`] as its popup, and usable on its own inline.
pub fn color_editor<M: Clone + 'static>(
    theme: &Theme,
    color: Rgba,
    hex: Option<&TextEdit>,
    on_event: impl Fn(ColorEvent) -> M + 'static,
) -> Div<M> {
    let on_event: Arc<dyn Fn(ColorEvent) -> M> = Arc::new(on_event);
    let channels = [
        ("R", color.r, theme.colors.axis_x),
        ("G", color.g, theme.colors.axis_y),
        ("B", color.b, theme.colors.axis_z),
        ("A", color.a, theme.colors.text_muted),
    ];
    let rows = channels
        .into_iter()
        .enumerate()
        .map(|(index, (label, value, tint))| {
            channel_row(theme, color, index, label, value, tint, on_event.clone())
        });
    v_flex()
        .w_px(EDITOR_WIDTH)
        .p(3)
        .gap(2.5)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .blocks_pointer()
        .child(color_swatch(color).w_full().h_px(36.0))
        .children(rows)
        .child(hex_row(theme, color, hex, on_event))
}

/// One channel of the editor: its label, its slider and its value out of 255.
fn channel_row<M: Clone + 'static>(
    theme: &Theme,
    color: Rgba,
    index: usize,
    label: &str,
    value: f32,
    tint: Rgba,
    on_event: Arc<dyn Fn(ColorEvent) -> M>,
) -> Div<M> {
    let (from, to) = (
        with_channel(color, index, 0.0),
        with_channel(color, index, 1.0),
    );
    let events = on_event;
    let control = slider(f64::from(value), 0.0, 1.0, move |event| match event {
        SliderEvent::Begin(v) => events(ColorEvent::Begin(with_channel(color, index, v as f32))),
        SliderEvent::Change(v) => events(ColorEvent::Change(with_channel(color, index, v as f32))),
        SliderEvent::Commit => events(ColorEvent::Commit),
        SliderEvent::Cancel => events(ColorEvent::Cancel),
    })
    .gradient(from, to);
    h_flex()
        .w_full()
        .gap(2)
        .items_center()
        .child(text(label.to_owned()).text_xs().color(tint).w_px(10.0))
        .child(control.flex_1())
        .child(
            text(format!("{}", (value * 255.0).round() as u32))
                .text_xs()
                .color(theme.colors.text_muted)
                .w_px(26.0),
        )
}

/// The hex code of the editor: shown, or a field while it is being typed.
fn hex_row<M: Clone + 'static>(
    theme: &Theme,
    color: Rgba,
    hex: Option<&TextEdit>,
    on_event: Arc<dyn Fn(ColorEvent) -> M>,
) -> Div<M> {
    let content: Box<dyn Element<M>> = Box::new(HexBox {
        shown: format_hex(color),
        edit: hex.cloned(),
        on_event,
        style: Style {
            width: Length::Full,
            ..Style::default()
        },
    });
    h_flex()
        .w_full()
        .gap(2)
        .items_center()
        .child(
            text("Hex")
                .text_xs()
                .color(theme.colors.text_muted)
                .w_px(36.0),
        )
        .child(v_flex().flex_1().child(content))
}

/// The box holding the hex code, clicked to type over it.
struct HexBox<M> {
    /// The code as it stands.
    shown: String,
    /// The code being typed, when it is.
    edit: Option<TextEdit>,
    /// What the editor sends.
    on_event: Arc<dyn Fn(ColorEvent) -> M>,
    /// How the box is sized.
    style: Style,
}

impl<M: 'static> Element<M> for HexBox<M> {
    /// How the box is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes the width it is offered and the height of a control.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        Size::new(available.width, cx.theme.size.control - 4.0)
    }

    /// Paints the box and the code in it, and takes the click or the caret press.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let theme = *cx.theme();
        let font = theme.text.sm.mono();
        let padding = 8.0;
        let inner = Rect::from_xywh(
            bounds.left() + padding,
            bounds.top(),
            (bounds.size.width - padding * 2.0).max(0.0),
            bounds.size.height,
        );
        let editing = self.edit.is_some();
        let value = match &self.edit {
            Some(edit) => edit.text().to_owned(),
            None => self.shown.clone(),
        };
        let action = if editing {
            let offsets = caret_offsets(cx, &value, font);
            let for_drag = offsets.clone();
            let (press, drag) = (self.on_event.clone(), self.on_event.clone());
            let left = inner.left();
            RegionAction::inert()
                .press(Arc::new(move |point: Point| {
                    press(ColorEvent::HexCaret(
                        index_at(&offsets, point.x - left),
                        false,
                    ))
                }))
                .drag(Arc::new(move |event: DragEvent| {
                    drag(ColorEvent::HexCaret(
                        index_at(&for_drag, event.position.x - left),
                        true,
                    ))
                }))
        } else {
            RegionAction::inert().click((self.on_event)(ColorEvent::HexEdit))
        };
        let interaction = cx.region(bounds, action);
        let border = match (editing, interaction.hovered) {
            (true, _) => theme.colors.border_focused,
            (false, true) => theme.colors.border_selected,
            (false, false) => theme.colors.border,
        };
        cx.quad(
            Quad::filled(bounds, theme.colors.surface_input)
                .corner_radius(theme.radius.md)
                .border(1.0, border),
        );
        let (caret, selection) = match &self.edit {
            Some(edit) => (edit.caret(), edit.selection()),
            None => (0, None),
        };
        paint_line(
            cx,
            inner,
            font,
            &Line {
                value: &value,
                placeholder: "",
                caret,
                selection,
                focused: editing,
                color: theme.colors.text,
            },
        );
    }
}
