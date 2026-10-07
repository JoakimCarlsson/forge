//! The one container: a flex box that stacks children along an axis.

use std::sync::Arc;

use fr_color::Rgba;
use fr_input::ScrollDelta;
use fr_math::{Point, Rect, Size};
use fr_render::Quad;

use crate::drag::DragEvent;
use crate::element::{Element, Interaction, IntoElement, LayoutContext, PaintContext};
use crate::rects::Rects;
use crate::region::RegionAction;
use crate::style::{Align, Axis, Justify, Length, Side, Style, Styled};

/// A container that measures its children, stacks them and paints a background.
pub struct Div<M> {
    /// How this container is sized, spaced and filled.
    style: Style,
    /// The children, in stacking order.
    children: Vec<Box<dyn Element<M>>>,
    /// What this container does with the pointer, when it answers to it at all.
    action: RegionAction<M>,
    /// The tooltip shown once the pointer rests on this container.
    tooltip: Option<String>,
    /// Where to leave this container's bounds, and under which key.
    record: Option<(Rects, String)>,
    /// Whether the pointer is over this container for the app's purposes even
    /// when it has no click message, as a panel over a scene is.
    blocks_pointer: bool,
    /// What the last measurement found, for painting to reuse.
    measured: Option<Measurement>,
}

/// What one measurement of a container found: the room its children were
/// offered, what each of them asked for, and the size that came to.
struct Measurement {
    /// The space inside the padding the children were measured against.
    content: Size,
    /// What each child measured to, before its cross extent was settled.
    children: Vec<Size>,
    /// The size the container reported for itself.
    size: Size,
}

impl Measurement {
    /// Whether children measured this way would measure the same again with
    /// `content` inside `bounds`, so painting need not ask them.
    ///
    /// Along each axis the room is either the room that was measured, or it
    /// is smaller only because the container was sized to what its children
    /// asked for and got exactly that. Children that asked for less than
    /// they were offered ask for the same again when offered only that much;
    /// a child that grows with its offer fills it, and a container around
    /// it is then never smaller than it was offered. Room that grew, or that
    /// shrank for any other reason, is measured again.
    fn holds_for(&self, content: Size, bounds: Size) -> bool {
        let axis_holds = |content: f32, measured: f32, bounds: f32, size: f32| {
            content == measured || (content < measured && bounds == size)
        };
        axis_holds(
            content.width,
            self.content.width,
            bounds.width,
            self.size.width,
        ) && axis_holds(
            content.height,
            self.content.height,
            bounds.height,
            self.size.height,
        )
    }
}

/// An empty container stacking children top to bottom.
pub fn div<M>() -> Div<M> {
    Div {
        style: Style::default(),
        children: Vec::new(),
        action: RegionAction::inert(),
        tooltip: None,
        record: None,
        blocks_pointer: false,
        measured: None,
    }
}

/// An empty container stacking children top to bottom.
pub fn v_flex<M>() -> Div<M> {
    div().column()
}

/// An empty container stacking children left to right.
pub fn h_flex<M>() -> Div<M> {
    div().row()
}

impl<M> Div<M> {
    /// Appends one child.
    pub fn child(mut self, child: impl IntoElement<M>) -> Self {
        self.children.push(child.into_element());
        self
    }

    /// Appends every child of `children`.
    pub fn children<I>(mut self, children: I) -> Self
    where
        I: IntoIterator,
        I::Item: IntoElement<M>,
    {
        self.children
            .extend(children.into_iter().map(IntoElement::into_element));
        self
    }

    /// Makes this container answer to a click by sending `message`.
    pub fn on_click(mut self, message: M) -> Self {
        self.action.click = Some(message);
        self
    }

    /// Makes this container answer to a second quick click by sending `message`.
    pub fn on_double_click(mut self, message: M) -> Self {
        self.action.double_click = Some(message);
        self
    }

    /// Makes this container send `message` when the secondary button goes down on it.
    pub fn on_secondary_click(mut self, message: M) -> Self {
        self.action.secondary = Some(message);
        self
    }

    /// Makes this container send the message `handler` builds from where the
    /// primary button went down on it.
    pub fn on_press(mut self, handler: impl Fn(Point) -> M + 'static) -> Self {
        self.action.press = Some(Arc::new(handler));
        self
    }

    /// Makes this container capture drags that start on it, sending the message
    /// `handler` builds from each event.
    pub fn on_drag(mut self, handler: impl Fn(DragEvent) -> M + 'static) -> Self {
        self.action.drag = Some(Arc::new(handler));
        self
    }

    /// Makes this container turn the wheel over it into the message `handler` builds.
    pub fn on_scroll(mut self, handler: impl Fn(ScrollDelta) -> M + 'static) -> Self {
        self.action.scroll = Some(Arc::new(handler));
        self
    }

    /// Shows `text` once the pointer has rested on this container.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// Leaves this container's bounds in `rects` under `key` as it paints.
    pub fn recorded(mut self, rects: &Rects, key: impl Into<String>) -> Self {
        self.record = Some((rects.clone(), key.into()));
        self
    }

    /// Makes this container a region the pointer cannot reach past, without
    /// answering to a click, so input over it never goes to what is behind it.
    pub fn blocks_pointer(mut self) -> Self {
        self.blocks_pointer = true;
        self
    }

    /// The fill for this container in `interaction`.
    fn background(&self, interaction: Interaction) -> Rgba {
        let style = &self.style;
        match (interaction.pressed, interaction.hovered) {
            (true, _) => style
                .background_active
                .or(style.background_hovered)
                .unwrap_or(style.background),
            (_, true) => style.background_hovered.unwrap_or(style.background),
            _ => style.background,
        }
    }

    /// Applies `build` only when `condition` holds.
    pub fn when(self, condition: bool, build: impl FnOnce(Self) -> Self) -> Self {
        if condition { build(self) } else { self }
    }

    /// Applies `build` with the value only when there is one.
    pub fn when_some<T>(self, value: Option<T>, build: impl FnOnce(Self, T) -> Self) -> Self {
        match value {
            Some(value) => build(self, value),
            None => self,
        }
    }

    /// Measures every child against the space inside the padding.
    ///
    /// Sizing on the stacking axis follows flexbox in two passes. First the
    /// children with an exact length or an `Auto` one are measured as they
    /// are; what is left over is then split between the children that grow —
    /// `Full` or a non-zero `flex_grow` — and each of those is measured
    /// against its own share, so a child that fills its parent's width cannot
    /// widen the parent it is being fitted into.
    ///
    /// The sizes returned are what the children asked for; how far across
    /// the stacking axis each one reaches is [`Self::settle_cross`]'s to say.
    fn measure_children(&mut self, content: Size, cx: &mut LayoutContext<'_>) -> Vec<Size> {
        let axis = self.style.axis;
        let gaps = self.style.gap * self.children.len().saturating_sub(1) as f32;

        let mut sizes = vec![Size::zero(); self.children.len()];
        let mut weights = vec![0.0; self.children.len()];

        for (index, child) in self.children.iter_mut().enumerate() {
            let style = child.layout_style();
            weights[index] = match main_length(&style, axis) {
                Length::Full => style.flex_grow.max(1.0),
                _ => style.flex_grow,
            };
            if weights[index] > 0.0 {
                continue;
            }

            sizes[index] = match main_length(&style, axis) {
                Length::Px(pixels) => {
                    let mut offer = content;
                    axis.set_main(&mut offer, pixels);
                    child.measure(offer, cx)
                }
                _ => child.measure(content, cx),
            };
        }

        let used: f32 = sizes.iter().map(|size| axis.main_of(*size)).sum();
        let leftover = (axis.main_of(content) - used - gaps).max(0.0);
        let total_weight: f32 = weights.iter().sum();

        for (index, child) in self.children.iter_mut().enumerate() {
            if weights[index] <= 0.0 {
                continue;
            }

            let share = if total_weight > 0.0 {
                leftover * weights[index] / total_weight
            } else {
                0.0
            };
            let mut offer = content;
            axis.set_main(&mut offer, share);
            let mut size = child.measure(offer, cx);
            axis.set_main(&mut size, share);
            sizes[index] = size;
        }

        sizes
    }

    /// Settles how far across the stacking axis each child reaches, given
    /// what it asked for in `sizes` and the room inside the padding.
    fn settle_cross(&self, sizes: &mut [Size], content: Size, stretch: bool) {
        let axis = self.style.axis;
        let align = self.style.align;
        for (index, size) in sizes.iter_mut().enumerate() {
            let style = self.children[index].layout_style();
            let cross = match cross_length(&style, axis) {
                Length::Px(pixels) => pixels,
                Length::Full => axis.cross_of(content),
                Length::Auto if align == Align::Stretch && stretch => axis.cross_of(content),
                Length::Auto => axis.cross_of(*size).min(axis.cross_of(content)),
            };
            axis.set_cross(size, cap_width(&style, axis, cross));
        }
    }

    /// What the children ask for inside `content` when painted into
    /// `bounds`, taken from the last measurement when that still holds.
    fn painted_children(
        &mut self,
        content: Size,
        bounds: Size,
        cx: &mut LayoutContext<'_>,
    ) -> Vec<Size> {
        match self.measured.take() {
            Some(measured) if measured.holds_for(content, bounds) => measured.children,
            _ => self.measure_children(content, cx),
        }
    }

    /// The space inside the padding, given the space this container is offered.
    fn content_offer(&self, available: Size) -> Size {
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            _ => self.capped_width(available.width),
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            _ => available.height,
        };

        Size::new(
            (width - self.style.padding.horizontal()).max(0.0),
            (height - self.style.padding.vertical()).max(0.0),
        )
    }

    /// Applies `min_width` and `max_width` to a width.
    fn capped_width(&self, width: f32) -> f32 {
        let width = match self.style.max_width {
            Some(max) => width.min(max),
            None => width,
        };
        match self.style.min_width {
            Some(min) => width.max(min),
            None => width,
        }
    }
}

impl<M> Styled for Div<M> {
    /// How this container is sized, spaced and filled.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: Clone> Element<M> for Div<M> {
    /// How this container is sized, spaced and filled.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Measures the children, then this container around them.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let content = self.content_offer(available);
        let children = self.measure_children(content, cx);
        let mut sizes = children.clone();
        self.settle_cross(&mut sizes, content, !self.style.fit_width);
        let axis = self.style.axis;

        let gaps = self.style.gap * sizes.len().saturating_sub(1) as f32;
        let main: f32 = sizes.iter().map(|size| axis.main_of(*size)).sum::<f32>() + gaps;
        let cross = sizes
            .iter()
            .map(|size| axis.cross_of(*size))
            .fold(0.0, f32::max);

        let mut intrinsic = Size::zero();
        axis.set_main(&mut intrinsic, main);
        axis.set_cross(&mut intrinsic, cross);

        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => self.capped_width(available.width),
            Length::Auto => self
                .capped_width(intrinsic.width + self.style.padding.horizontal())
                .min(available.width.max(0.0)),
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            Length::Full => available.height,
            Length::Auto => intrinsic.height + self.style.padding.vertical(),
        };

        let size = Size::new(width, height);
        self.measured = Some(Measurement {
            content,
            children,
            size,
        });
        size
    }

    /// Paints the background, then places and paints every child.
    ///
    /// The children are placed from what they asked for when this container
    /// was measured, and measured again only when the bounds it was given
    /// could change their answer.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        if let Some((rects, key)) = &self.record {
            rects.set(key.clone(), bounds);
        }
        let interaction = if self.action.is_active() {
            cx.region(bounds, self.action.clone())
        } else if self.blocks_pointer {
            cx.block(bounds)
        } else {
            Interaction::default()
        };
        if let Some(text) = &self.tooltip
            && interaction.hovered
        {
            cx.tooltip(bounds, text.clone());
        }

        cx.quad(
            Quad::filled(bounds, self.background(interaction))
                .corner_radii(self.style.corner_radii)
                .border(self.style.border_width, self.style.border_color),
        );
        for (side, (width, color)) in Side::ALL.into_iter().zip(self.style.sides) {
            if width > 0.0 && !color.is_transparent() {
                cx.quad(Quad::filled(side.strip(bounds, width), color));
            }
        }

        let padding = self.style.padding;
        let content = Rect::from_xywh(
            bounds.left() + padding.left,
            bounds.top() + padding.top,
            (bounds.size.width - padding.horizontal()).max(0.0),
            (bounds.size.height - padding.vertical()).max(0.0),
        );

        if self.style.overflow_hidden {
            cx.push_clip(bounds);
        }

        let mut sizes = self.painted_children(content.size, bounds.size, &mut cx.layout);
        self.settle_cross(&mut sizes, content.size, true);
        let axis = self.style.axis;
        let gap = self.style.gap;
        let gaps = gap * sizes.len().saturating_sub(1) as f32;
        let used: f32 = sizes.iter().map(|size| axis.main_of(*size)).sum::<f32>() + gaps;
        let leftover = (axis.main_of(content.size) - used).max(0.0);

        let mut main = match self.style.justify {
            Justify::Start | Justify::Between => 0.0,
            Justify::Center => leftover / 2.0,
            Justify::End => leftover,
        };
        let spread = match self.style.justify {
            Justify::Between if sizes.len() > 1 => leftover / (sizes.len() - 1) as f32,
            _ => 0.0,
        };

        for (child, size) in self.children.iter_mut().zip(sizes) {
            let room = axis.cross_of(content.size) - axis.cross_of(size);
            let cross = if child.layout_style().center_horizontally {
                (room / 2.0).max(0.0)
            } else {
                match self.style.align {
                    Align::Start | Align::Stretch => 0.0,
                    Align::Center => (room / 2.0).max(0.0),
                    Align::End => room.max(0.0),
                }
            };

            let origin = match axis {
                Axis::Horizontal => (content.left() + main, content.top() + cross),
                Axis::Vertical => (content.left() + cross, content.top() + main),
            };
            child.paint(
                Rect::from_xywh(origin.0, origin.1, size.width, size.height),
                cx,
            );

            main += axis.main_of(size) + gap + spread;
        }

        if self.style.overflow_hidden {
            cx.pop_clip();
        }
    }
}

/// The length that sizes an element along `axis`.
fn main_length(style: &Style, axis: Axis) -> Length {
    match axis {
        Axis::Horizontal => style.width,
        Axis::Vertical => style.height,
    }
}

/// Applies a child's `max_width` to its cross extent, when that is its width.
fn cap_width(style: &Style, axis: Axis, cross: f32) -> f32 {
    match (axis, style.max_width) {
        (Axis::Vertical, Some(max)) => cross.min(max),
        _ => cross,
    }
}

/// The length that sizes an element across `axis`.
fn cross_length(style: &Style, axis: Axis) -> Length {
    match axis {
        Axis::Horizontal => style.height,
        Axis::Vertical => style.width,
    }
}
