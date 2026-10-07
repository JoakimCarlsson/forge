//! A scrolling area: a window onto taller content, with a wheel and a thumb.
//!
//! The content is laid out as tall as it wants and painted shifted up by an
//! offset, clipped to the area. How far it is shifted is a [`Scroll`] the caller
//! keeps, and the area writes the extents it found back into it, so the offset
//! is held against what was really there. The wheel and the thumb come back as
//! the caller's own messages carrying a [`ScrollEvent`], which the caller hands
//! to [`Scroll::apply`].

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use fr_math::{Rect, Size};
use fr_render::Quad;

use crate::drag::{DragEvent, DragPhase};
use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::region::RegionAction;
use crate::style::{Axis, Style, Styled};

/// Width of the thumb.
const THUMB_WIDTH: f32 = 6.0;

/// How far the thumb's track sits from the edges of what it scrolls.
const THUMB_PADDING: f32 = 4.0;

/// Shortest the thumb is drawn, however long the content runs.
const MIN_THUMB: f32 = 25.0;

/// The room, with the space either side of it, that a thumb takes along the
/// right edge of an area: content kept this far in never runs under the thumb.
pub const SCROLLBAR_GUTTER: f32 = THUMB_WIDTH + THUMB_PADDING * 2.0;

/// What a scroll area is scrolled by, as the caller's message carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollEvent {
    /// The wheel moved the content down by this many pixels, which is the view
    /// towards the top when positive.
    Wheel(f32),
    /// The thumb was dragged to put the view at this offset.
    To(f32),
}

/// The measures of one scrolling area.
#[derive(Clone, Copy, Debug, Default)]
struct ScrollState {
    /// How far the content is shifted up, in logical pixels.
    offset: f32,
    /// The height the last frame painted the content at.
    content: f32,
    /// The height of the window the content is seen through.
    viewport: f32,
}

impl ScrollState {
    /// Holds the offset between the top and the end of the content.
    fn clamped(mut self) -> Self {
        let limit = (self.content - self.viewport).max(0.0);
        self.offset = self.offset.clamp(0.0, limit);
        self
    }
}

/// How far one scrolling area is scrolled, shared with the area that draws it.
///
/// Cloning gives another handle on the same offset. The caller keeps one per
/// area, so two of them side by side scroll independently.
#[derive(Clone, Debug, Default)]
pub struct Scroll {
    /// The offset and extents, written by the area and by the caller.
    state: Rc<Cell<ScrollState>>,
}

impl Scroll {
    /// A scroll at the top.
    pub fn new() -> Self {
        Self::default()
    }

    /// A scroll `offset` pixels down, held against the content once a frame has painted it.
    pub fn at(offset: f32) -> Self {
        let scroll = Self::new();
        scroll.state.set(ScrollState {
            offset,
            ..ScrollState::default()
        });
        scroll
    }

    /// How far down the content the view begins.
    pub fn offset(&self) -> f32 {
        self.state.get().offset
    }

    /// The height of the whole content as of the last frame.
    pub fn content_height(&self) -> f32 {
        self.state.get().content
    }

    /// The height of the window the content is seen through as of the last frame.
    pub fn viewport_height(&self) -> f32 {
        self.state.get().viewport
    }

    /// The furthest the view can go: how much of the content is out of sight.
    pub fn max_offset(&self) -> f32 {
        let state = self.state.get();
        (state.content - state.viewport).max(0.0)
    }

    /// Puts the view at `offset`, held between the top and the end.
    pub fn scroll_to(&self, offset: f32) {
        let mut state = self.state.get();
        state.offset = offset;
        self.state.set(state.clamped());
    }

    /// Moves the view down by `pixels`, up when negative.
    pub fn scroll_by(&self, pixels: f32) {
        self.scroll_to(self.offset() + pixels);
    }

    /// Moves the view just far enough to show the span from `top` to `bottom`,
    /// measured from the top of the content.
    pub fn ensure_visible(&self, top: f32, bottom: f32) {
        let state = self.state.get();
        if top < state.offset {
            self.scroll_to(top);
        } else if bottom > state.offset + state.viewport {
            self.scroll_to(bottom - state.viewport);
        }
    }

    /// Applies what a scroll area reported.
    pub fn apply(&self, event: ScrollEvent) {
        match event {
            ScrollEvent::Wheel(delta) => self.scroll_by(-delta),
            ScrollEvent::To(offset) => self.scroll_to(offset),
        }
    }

    /// Records the extents a frame found, holding the offset against them.
    fn set_extents(&self, viewport: f32, content: f32) {
        let mut state = self.state.get();
        state.viewport = viewport;
        state.content = content;
        self.state.set(state.clamped());
    }
}

/// What a scroll area reports through, as the caller's message.
type OnScroll<M> = Arc<dyn Fn(ScrollEvent) -> M>;

/// An area that shows its child through a window, scrolled by a [`Scroll`].
pub struct ScrollArea<M> {
    /// The scroll the area is drawn at, and records its extents in.
    scroll: Scroll,
    /// How the area itself is sized.
    style: Style,
    /// What is scrolled.
    child: Box<dyn Element<M>>,
    /// What the wheel and the thumb send, when the caller listens.
    on_scroll: Option<OnScroll<M>>,
}

/// `child`, scrolled by `scroll` inside whatever room the area is given.
pub fn scroll_area<M>(scroll: &Scroll, child: impl IntoElement<M>) -> ScrollArea<M> {
    ScrollArea {
        scroll: scroll.clone(),
        style: Style::default(),
        child: child.into_element(),
        on_scroll: None,
    }
}

impl<M> ScrollArea<M> {
    /// Returns this area sending `on_scroll` for the wheel and the thumb.
    ///
    /// Without it the area is still drawn at its offset, but nothing moves it.
    pub fn on_scroll(mut self, on_scroll: impl Fn(ScrollEvent) -> M + 'static) -> Self {
        self.on_scroll = Some(Arc::new(on_scroll));
        self
    }
}

impl<M> Styled for ScrollArea<M> {
    /// How the area is sized in its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for ScrollArea<M> {
    /// How the area is sized in its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// The whole of what it is offered: the content scrolls rather than grows.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let width = match self.style.width {
            crate::style::Length::Px(pixels) => pixels,
            _ => available.width,
        };
        let height = match self.style.height {
            crate::style::Length::Px(pixels) => pixels,
            _ => available.height,
        };
        Size::new(width, height)
    }

    /// Paints the child at the scrolled offset, clipped to the area, and the thumb.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let content = self.child.measure(bounds.size, &mut cx.layout);
        self.scroll.set_extents(bounds.size.height, content.height);
        let offset = self.scroll.offset();

        if let Some(on_scroll) = self.on_scroll.clone() {
            let wheel = on_scroll.clone();
            cx.region(
                bounds,
                RegionAction::inert()
                    .scroll(Arc::new(move |delta| wheel(ScrollEvent::Wheel(delta.y)))),
            );
        }

        cx.push_clip(bounds);
        self.child.paint(
            Rect::from_xywh(
                bounds.left(),
                bounds.top() - offset,
                bounds.size.width,
                content.height.max(bounds.size.height),
            ),
            cx,
        );
        cx.pop_clip();

        if let Some(on_scroll) = self.on_scroll.clone() {
            paint_thumb(cx, bounds, content.height, offset, on_scroll);
        }
    }
}

/// Draws and registers the thumb for content `total` tall seen through `bounds`
/// at `offset`, when the content runs past the view.
fn paint_thumb<M: 'static>(
    cx: &mut PaintContext<'_, '_, M>,
    bounds: Rect,
    total: f32,
    offset: f32,
    on_scroll: OnScroll<M>,
) {
    let showing = bounds.size.height;
    let hidden = total - showing;
    if hidden <= 0.0 {
        return;
    }
    let track = Rect::from_xywh(
        bounds.right() - THUMB_PADDING - THUMB_WIDTH,
        bounds.top() + THUMB_PADDING,
        THUMB_WIDTH,
        (showing - THUMB_PADDING * 2.0).max(0.0),
    );
    let extent = (track.size.height * showing / total).max(MIN_THUMB);
    let travel = (track.size.height - extent).max(0.0);
    let reached = (offset / hidden).clamp(0.0, 1.0);
    let thumb = Rect::from_xywh(
        track.left(),
        track.top() + travel * reached,
        track.size.width,
        extent,
    );
    let step = if travel > 0.0 { hidden / travel } else { 1.0 };
    let grab = thumb.outset(2.0);
    let interaction = cx.region(
        grab,
        RegionAction::inert().drag(Arc::new(move |event: DragEvent| {
            let moved = match event.phase {
                DragPhase::Cancelled => 0.0,
                _ => event.total_along(Axis::Vertical) * step,
            };
            on_scroll(ScrollEvent::To(offset + moved))
        })),
    );
    let theme = *cx.theme();
    let strength = if interaction.hovered || interaction.pressed {
        theme.emphasis.scrollbar_active
    } else {
        theme.emphasis.scrollbar
    };
    cx.quad(
        Quad::filled(thumb, theme.colors.text_subtle.alpha(strength))
            .corner_radius(theme.radius.full),
    );
}
