//! A box with one line of text being typed into, with a caret in it.
//!
//! The field draws and nothing more: what the text is, where the caret sits,
//! which characters are selected and whether the keyboard is going to this field
//! are the caller's, kept in a [`TextEdit`]. A press reports the character the
//! pointer landed before, so the caller can move its own caret there, and a drag
//! keeps reporting it so the caller can extend the selection.

use std::ops::Range;
use std::sync::Arc;

use fr_color::Rgba;
use fr_math::{Point, Rect, Size};
use fr_render::{FontStyle, Quad};

use crate::drag::DragEvent;
use crate::element::{Element, Interaction, LayoutContext, PaintContext};
use crate::region::RegionAction;
use crate::style::{Edges, Length, Style, Styled, space};
use crate::theme::{Font, TextSize};
use crate::widgets::TextEdit;

/// Width of the caret drawn while the field has the keyboard.
const CARET_WIDTH: f32 = 1.5;

/// What a press or drag in the field sends, given the character it landed before.
type OnIndex<M> = Arc<dyn Fn(usize) -> M>;

/// One line of editable text in an input box, with a caret where the cursor is.
pub struct Field<M> {
    /// What is in the field.
    value: String,
    /// What is shown instead while it is empty.
    placeholder: String,
    /// How many characters into the value the caret sits.
    caret: usize,
    /// The characters washed in the selection colour, when a span is selected.
    selection: Option<Range<usize>>,
    /// Whether keystrokes are going to this field.
    focused: bool,
    /// The step of the scale the text is drawn at.
    font: Font,
    /// What a press in the field sends.
    on_press: Option<OnIndex<M>>,
    /// What a drag that starts in the field sends.
    on_drag: Option<OnIndex<M>>,
    /// Whether the input box is drawn around the text.
    boxed: bool,
    /// How the field is sized and padded.
    style: Style,
}

/// A field holding `value`, with the caret `caret` characters into it.
pub fn field<M>(value: impl Into<String>, caret: usize, focused: bool) -> Field<M> {
    Field {
        value: value.into(),
        placeholder: String::new(),
        caret,
        selection: None,
        focused,
        font: Font::new(TextSize::Sm),
        on_press: None,
        on_drag: None,
        boxed: true,
        style: Style {
            width: Length::Full,
            ..Style::default()
        },
    }
}

/// A field showing `edit`, with its caret and selection.
pub fn text_field<M>(edit: &TextEdit, focused: bool) -> Field<M> {
    field(edit.text(), edit.caret(), focused).selection(edit.selection())
}

impl<M> Field<M> {
    /// Returns this field showing `placeholder` while it is empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Returns this field drawn in the monospaced family.
    pub fn font_mono(mut self) -> Self {
        self.font = self.font.mono();
        self
    }

    /// Returns this field reporting presses through `on_press`.
    pub fn on_press(mut self, on_press: impl Fn(usize) -> M + 'static) -> Self {
        self.on_press = Some(Arc::new(on_press));
        self
    }

    /// Returns this field reporting a drag that starts in it through `on_drag`.
    pub fn on_drag(mut self, on_drag: impl Fn(usize) -> M + 'static) -> Self {
        self.on_drag = Some(Arc::new(on_drag));
        self
    }

    /// Returns this field with characters `selection` washed in the selection colour.
    pub fn selection(mut self, selection: Option<Range<usize>>) -> Self {
        self.selection = selection;
        self
    }

    /// Returns this field without the box: only the text, caret and selection.
    pub fn bare(mut self) -> Self {
        self.boxed = false;
        self
    }
}

impl<M> Styled for Field<M> {
    /// How the field is sized and padded.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for Field<M> {
    /// How the field is sized and padded.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Reports the width it is offered and the height of a control.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let font = self.font.resolve(&cx.theme.text);
        let run = cx.measure(&self.value, font);
        let padding = self.padding();
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => available.width,
            Length::Auto => run.width + padding.horizontal(),
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            Length::Full => available.height,
            Length::Auto if self.boxed => cx.theme.size.control,
            Length::Auto => run.height + padding.vertical(),
        };
        Size::new(width, height)
    }

    /// Paints the box, the text, the selection and the caret, and takes the press.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let theme = *cx.theme();
        let padding = self.padding();
        let font = self.font.resolve(&theme.text);
        let interaction = self.register(bounds, padding, font, cx);
        if self.boxed {
            let border = match (self.focused, interaction.hovered) {
                (true, _) => theme.colors.border_focused,
                (false, true) => theme.colors.border_selected,
                (false, false) => theme.colors.border,
            };
            cx.quad(
                Quad::filled(bounds, theme.colors.surface_input)
                    .corner_radius(theme.radius.md)
                    .border(1.0, border),
            );
        }
        let inner = Rect::from_xywh(
            bounds.left() + padding.left,
            bounds.top() + padding.top,
            (bounds.size.width - padding.horizontal()).max(0.0),
            (bounds.size.height - padding.vertical()).max(0.0),
        );
        paint_line(
            cx,
            inner,
            font,
            &Line {
                value: &self.value,
                placeholder: &self.placeholder,
                caret: self.caret,
                selection: self.selection.clone(),
                focused: self.focused,
                color: theme.colors.text,
            },
        );
    }
}

impl<M: 'static> Field<M> {
    /// The padding the text sits in: the style's own, or the box's default.
    fn padding(&self) -> Edges {
        let mut padding = self.style.padding;
        if self.boxed && padding.left == 0.0 && padding.right == 0.0 {
            padding.left = space(2.5);
            padding.right = space(2.5);
        }
        padding
    }

    /// Registers the region that reports presses and drags as character indices.
    fn register(
        &self,
        bounds: Rect,
        padding: Edges,
        font: FontStyle,
        cx: &mut PaintContext<'_, '_, M>,
    ) -> Interaction {
        if self.on_press.is_none() && self.on_drag.is_none() {
            return cx.block(bounds);
        }
        let offsets = caret_offsets(cx, &self.value, font);
        let inner = (bounds.size.width - padding.horizontal()).max(0.0);
        let shift = shift_for(&offsets, self.caret, inner, self.focused);
        let left = bounds.left() + padding.left - shift;
        let mut action = RegionAction::inert();
        if let Some(on_press) = self.on_press.clone() {
            let offsets = offsets.clone();
            action = action.press(Arc::new(move |point: Point| {
                on_press(index_at(&offsets, point.x - left))
            }));
        }
        if let Some(on_drag) = self.on_drag.clone() {
            action = action.drag(Arc::new(move |event: DragEvent| {
                on_drag(index_at(&offsets, event.position.x - left))
            }));
        }
        cx.region(bounds, action)
    }
}

/// The text of one line and how to draw it.
pub(crate) struct Line<'a> {
    /// What the line says.
    pub(crate) value: &'a str,
    /// What is shown in its place while it is empty.
    pub(crate) placeholder: &'a str,
    /// Where the caret is, in characters.
    pub(crate) caret: usize,
    /// The selected characters.
    pub(crate) selection: Option<Range<usize>>,
    /// Whether the caret is drawn.
    pub(crate) focused: bool,
    /// The colour of the text.
    pub(crate) color: Rgba,
}

/// How far to shift a line left so the caret at `caret` stays inside `inner` pixels.
pub(crate) fn shift_for(offsets: &[f32], caret: usize, inner: f32, focused: bool) -> f32 {
    if !focused {
        return 0.0;
    }
    let last = offsets.len().saturating_sub(1);
    let at = offsets.get(caret.min(last)).copied().unwrap_or(0.0);
    (at - inner + CARET_WIDTH).max(0.0)
}

/// How far into the line a caret sits before each character, and after the last.
pub(crate) fn caret_offsets<M>(
    cx: &mut PaintContext<'_, '_, M>,
    value: &str,
    font: FontStyle,
) -> Vec<f32> {
    let mut offsets = Vec::with_capacity(value.chars().count() + 1);
    let mut prefix = String::new();
    offsets.push(0.0);
    for character in value.chars() {
        prefix.push(character);
        offsets.push(cx.measure(&prefix, font).width);
    }
    offsets
}

/// The index of the caret position nearest `x` pixels into the line.
pub(crate) fn index_at(offsets: &[f32], x: f32) -> usize {
    offsets
        .iter()
        .enumerate()
        .min_by(|(_, one), (_, other)| (*one - x).abs().total_cmp(&(*other - x).abs()))
        .map_or(0, |(index, _)| index)
}

/// Paints `line` vertically centred in `inner`, clipped to it, with its
/// selection wash and caret.
pub(crate) fn paint_line<M>(
    cx: &mut PaintContext<'_, '_, M>,
    inner: Rect,
    font: FontStyle,
    line: &Line<'_>,
) {
    let theme = *cx.theme();
    let empty = line.value.is_empty();
    let shown = if empty { line.placeholder } else { line.value };
    let offsets = caret_offsets(cx, line.value, font);
    let shift = shift_for(&offsets, line.caret, inner.size.width, line.focused);
    let top = inner.top() + ((inner.size.height - font.line_height) / 2.0).round();
    let origin = Point::new(inner.left() - shift, top);

    cx.push_clip(inner.outset(1.0));
    if let Some(selected) = &line.selection {
        let last = offsets.len().saturating_sub(1);
        let start = offsets[selected.start.min(last)];
        let end = offsets[selected.end.min(last)];
        if end > start {
            cx.quad(Quad::filled(
                Rect::from_xywh(origin.x + start, top, end - start, font.line_height),
                theme.colors.selection.alpha(theme.emphasis.selection),
            ));
        }
    }
    if !shown.is_empty() {
        let color = if empty {
            theme.colors.text_subtle
        } else {
            line.color
        };
        let run = cx.shape(shown, font);
        cx.text(origin, run, color);
    }
    if line.focused {
        let last = offsets.len().saturating_sub(1);
        let offset = offsets[line.caret.min(last)];
        cx.quad(Quad::filled(
            Rect::from_xywh(origin.x + offset, top, CARET_WIDTH, font.line_height),
            theme.colors.cursor,
        ));
    }
    cx.pop_clip();
}
