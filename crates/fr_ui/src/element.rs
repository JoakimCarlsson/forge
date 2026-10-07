//! The element tree: what an element is, and the two passes it goes through.
//!
//! The tree is rebuilt every frame. An element is a value the caller assembles,
//! measured once and painted once, then dropped; nothing of it survives to the
//! next frame. What does survive lives in the caller's own state and in the
//! shaping cache behind [`LayoutContext::measure`], which is why rebuilding is
//! cheap enough to do at the refresh rate.

use std::sync::Arc;

use fr_color::Rgba;
use fr_math::{Point, Rect, Size};
use fr_render::{FontStyle, Quad, ShapedRun, Svg, TextSystem};

use crate::canvas::Canvas;
use crate::region::{Region, RegionAction};
use crate::style::Style;
use crate::theme::Theme;

/// What the pointer is doing, as of the last event.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Where the pointer is, if it is over the window at all.
    pub pointer: Option<Point>,
    /// Where the pointer went down, while it is still held.
    pub pressed_at: Option<Point>,
    /// The bounds of the region that has captured the held pointer for a drag.
    pub drag_bounds: Option<Rect>,
    /// Whether a drag has begun and holds the pointer.
    pub dragging: bool,
}

impl Input {
    /// Whether the pointer is inside `bounds`.
    pub fn is_over(&self, bounds: Rect) -> bool {
        self.pointer.is_some_and(|pointer| bounds.contains(pointer))
    }

    /// Whether the pointer went down inside `bounds` and is still held there.
    pub fn is_pressing(&self, bounds: Rect) -> bool {
        self.pressed_at
            .is_some_and(|pressed_at| bounds.contains(pressed_at))
            && (self.is_over(bounds) || self.drag_bounds == Some(bounds))
    }
}

/// What an interactive element needs to know to paint itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct Interaction {
    /// The pointer is over the element, and nothing painted above it is.
    pub hovered: bool,
    /// The pointer is held down on the element.
    pub pressed: bool,
    /// The element holds keyboard focus.
    pub focused: bool,
}

/// Measurement: the theme to size against and the text system to measure with.
pub struct LayoutContext<'a> {
    /// The tokens this frame is drawn from.
    pub theme: &'a Theme,
    /// Shaping and measurement, cached across frames.
    text: &'a mut TextSystem,
}

impl<'a> LayoutContext<'a> {
    /// Creates a context measuring against `theme` with `text`.
    pub fn new(theme: &'a Theme, text: &'a mut TextSystem) -> Self {
        Self { theme, text }
    }

    /// Shapes `content` in `font`, reusing the cached run when there is one.
    pub fn shape(&mut self, content: &str, font: FontStyle) -> Arc<ShapedRun> {
        self.text.shape(content, font)
    }

    /// The extent `content` occupies in `font`.
    pub fn measure(&mut self, content: &str, font: FontStyle) -> Size {
        self.text.measure(content, font)
    }
}

/// Painting: everything measurement has, plus the canvas and the input.
pub struct PaintContext<'a, 'b, M> {
    /// Measurement, which painting needs as much as layout does.
    pub layout: LayoutContext<'b>,
    /// The canvas this frame's primitives are recorded on.
    canvas: &'a mut Canvas,
    /// What the pointer is doing.
    input: Input,
    /// The region holding keyboard focus, as an index into `regions`.
    focused: Option<usize>,
    /// The layer of the topmost region under the pointer in the last frame.
    pointer_layer: u32,
    /// The regions painted so far this frame.
    regions: &'a mut Vec<Region<M>>,
    /// The tooltip the hovered element asked for, with where it was painted.
    tooltip: Option<(Rect, String)>,
}

impl<'a, 'b, M> PaintContext<'a, 'b, M> {
    /// Creates a paint context writing into `canvas` and `regions`.
    pub(crate) fn new(
        layout: LayoutContext<'b>,
        canvas: &'a mut Canvas,
        input: Input,
        focused: Option<usize>,
        pointer_layer: u32,
        regions: &'a mut Vec<Region<M>>,
    ) -> Self {
        Self {
            layout,
            canvas,
            input,
            focused,
            pointer_layer,
            regions,
            tooltip: None,
        }
    }

    /// The tokens this frame is drawn from.
    pub fn theme(&self) -> &Theme {
        self.layout.theme
    }

    /// Shapes `content` in `font`, reusing the cached run when there is one.
    pub fn shape(&mut self, content: &str, font: FontStyle) -> Arc<ShapedRun> {
        self.layout.shape(content, font)
    }

    /// The extent `content` occupies in `font`.
    pub fn measure(&mut self, content: &str, font: FontStyle) -> Size {
        self.layout.measure(content, font)
    }

    /// Adds a quad to the frame.
    pub fn quad(&mut self, quad: Quad) {
        self.canvas.quad(quad);
    }

    /// Draws `svg` tinted `color` inside `bounds`, turned `rotation` radians
    /// clockwise around its centre.
    pub fn rotated_icon(&mut self, bounds: Rect, svg: Svg, color: Rgba, rotation: f32) {
        self.canvas.icon(bounds, svg, color, rotation);
    }

    /// Draws a shaped run with its line box starting at `origin`.
    pub fn text(&mut self, origin: Point, run: Arc<ShapedRun>, color: Rgba) {
        self.canvas.text(origin, run, color);
    }

    /// Confines later primitives and regions to `bounds` as well as the current clip.
    pub fn push_clip(&mut self, bounds: Rect) {
        self.canvas.push_clip(bounds);
    }

    /// Restores the clip in force before the matching [`Self::push_clip`].
    pub fn pop_clip(&mut self) {
        self.canvas.pop_clip();
    }

    /// The clip in force: the part of the window later painting can show.
    pub fn clip(&self) -> Rect {
        self.canvas.clip()
    }

    /// Draws later primitives, and registers later regions, over everything painted so far.
    ///
    /// A layer is clipped to the window alone, whatever clip its owner was
    /// painted under, and blocks the pointer from reaching the layers below.
    pub fn push_layer(&mut self) {
        self.canvas.push_layer();
    }

    /// Returns to the layer in force before the matching [`Self::push_layer`].
    pub fn pop_layer(&mut self) {
        self.canvas.pop_layer();
    }

    /// What the pointer is doing.
    pub fn input(&self) -> Input {
        self.input
    }

    /// The window this frame is being drawn for.
    pub fn viewport(&self) -> Rect {
        self.canvas.viewport()
    }

    /// Whether the pointer is over the part of `bounds` the clip leaves in sight.
    pub fn pointer_over(&self, bounds: Rect) -> bool {
        self.input.is_over(bounds.intersect(self.canvas.clip()))
    }

    /// Records the tooltip `text` for the element at `bounds`, to show once the
    /// pointer has rested on it.
    pub fn tooltip(&mut self, bounds: Rect, text: String) {
        self.tooltip = Some((bounds, text));
    }

    /// Takes the tooltip the hovered element asked for this frame, if any.
    pub(crate) fn take_tooltip(&mut self) -> Option<(Rect, String)> {
        self.tooltip.take()
    }

    /// Registers `bounds` as a click and tab target sending `message`.
    ///
    /// The returned state is what the element paints itself from: hover and
    /// press come from the current pointer position, focus from the tab index
    /// this registration takes.
    pub fn interactive(&mut self, bounds: Rect, message: M) -> Interaction {
        self.region(bounds, RegionAction::inert().click(message))
    }

    /// Registers `bounds` as a region that sends `on_click` when it is clicked.
    ///
    /// A region without a message is still a region: it is a thing the pointer
    /// can be over, and it keeps whatever lies under it from being clicked.
    pub fn clickable(&mut self, bounds: Rect, on_click: Option<M>) -> Interaction {
        let action = match on_click {
            Some(message) => RegionAction::inert().click(message),
            None => RegionAction::inert(),
        };
        self.region(bounds, action)
    }

    /// Registers `bounds` as a region that does nothing and blocks what is under it.
    pub fn block(&mut self, bounds: Rect) -> Interaction {
        self.region(bounds, RegionAction::inert())
    }

    /// Registers `bounds` as a region answering to the pointer as `action` says.
    ///
    /// The region is cut down to the clip in force, so content scrolled out of
    /// sight cannot be pressed, and sits on the layer being painted.
    pub fn region(&mut self, bounds: Rect, action: RegionAction<M>) -> Interaction {
        let bounds = bounds.intersect(self.canvas.clip());
        let layer = self.canvas.layer();
        let index = self.regions.len();
        self.regions.push(Region {
            bounds,
            layer,
            action,
        });
        self.interaction(bounds, layer, index)
    }

    /// How the pointer and focus stand toward the region at `index`.
    ///
    /// A region under a layer painted above it last frame is not hovered, and
    /// while a drag holds the pointer only the region that holds it is.
    fn interaction(&self, bounds: Rect, layer: u32, index: usize) -> Interaction {
        let captured = self.input.drag_bounds == Some(bounds);
        let hovered = self.input.is_over(bounds)
            && layer >= self.pointer_layer
            && (!self.input.dragging || captured);
        Interaction {
            hovered,
            pressed: self.input.is_pressing(bounds),
            focused: self.focused == Some(index),
        }
    }
}

/// One node of the tree: measured against an offer, then painted into bounds.
///
/// `M` is the caller's message type. An element never mutates the caller's
/// state; it registers interest through [`PaintContext::interactive`] and the
/// window turns a click or a keypress into one message the caller applies.
pub trait Element<M> {
    /// The style the parent lays this element out with.
    fn layout_style(&self) -> Style {
        Style::default()
    }

    /// The extent this element wants, given the extent it is offered.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size;

    /// Paints this element into `bounds`.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>);
}

/// Anything that can become a child of a container.
///
/// Elements convert to themselves; the trait exists so containers can accept a
/// boxed child, an element by value, or a widget that wraps one, all with the
/// same `child` call.
pub trait IntoElement<M> {
    /// Boxes this value as a child element.
    fn into_element(self) -> Box<dyn Element<M>>;
}

impl<M, E> IntoElement<M> for E
where
    E: Element<M> + 'static,
{
    /// Boxes the element itself.
    fn into_element(self) -> Box<dyn Element<M>> {
        Box::new(self)
    }
}

impl<M> Element<M> for Box<dyn Element<M>> {
    /// Defers to the boxed element.
    fn layout_style(&self) -> Style {
        (**self).layout_style()
    }

    /// Defers to the boxed element.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        (**self).measure(available, cx)
    }

    /// Defers to the boxed element.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        (**self).paint(bounds, cx);
    }
}
