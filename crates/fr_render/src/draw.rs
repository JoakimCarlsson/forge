//! The draw list: what a caller submits for one frame.

use std::sync::Arc;

use crate::svg::Svg;
use crate::text::ShapedRun;
use fr_color::Rgba;
use fr_math::{Point, Rect, Size};

/// A rounded, optionally bordered rectangle.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    /// Where the quad sits, in logical pixels.
    pub bounds: Rect,
    /// Fill colour inside the border.
    pub background: Rgba,
    /// Corner radii, clockwise from the top-left corner.
    pub corner_radii: [f32; 4],
    /// Thickness of the border, drawn inside `bounds`.
    pub border_width: f32,
    /// Colour of the border.
    pub border_color: Rgba,
}

impl Quad {
    /// Creates a filled quad with square corners and no border.
    pub fn filled(bounds: Rect, background: Rgba) -> Self {
        Self {
            bounds,
            background,
            corner_radii: [0.0; 4],
            border_width: 0.0,
            border_color: Rgba::TRANSPARENT,
        }
    }

    /// Returns this quad with every corner rounded by `radius`.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radii = [radius; 4];
        self
    }

    /// Returns this quad with each corner rounded on its own.
    ///
    /// The radii run clockwise from the top-left corner, the way CSS writes
    /// them, so a shape can be flat where it meets its neighbour.
    pub fn corner_radii(mut self, radii: [f32; 4]) -> Self {
        self.corner_radii = radii;
        self
    }

    /// Returns this quad with a border of `width` in `color`.
    pub fn border(mut self, width: f32, color: Rgba) -> Self {
        self.border_width = width;
        self.border_color = color;
        self
    }

    /// Whether this quad would put no pixels on the screen.
    pub fn is_invisible(&self) -> bool {
        let no_fill = self.background.is_transparent();
        let no_border = self.border_width <= 0.0 || self.border_color.is_transparent();
        no_fill && no_border
    }
}

/// A shaped run placed on the screen in one colour.
#[derive(Clone)]
pub struct TextRun {
    /// Top-left corner of the run's line box.
    pub origin: Point,
    /// The shaped glyphs.
    pub run: Arc<ShapedRun>,
    /// Colour every glyph is drawn in.
    pub color: Rgba,
}

/// One icon placed on the screen in one colour.
#[derive(Clone, Copy)]
pub struct IconRun {
    /// The square the icon is drawn inside.
    pub bounds: Rect,
    /// The artwork to draw.
    pub svg: Svg,
    /// Colour the artwork's coverage is tinted with.
    pub color: Rgba,
    /// Clockwise rotation around the icon's centre, in radians.
    pub rotation: f32,
}

/// One frame's worth of primitives, in submission order.
///
/// A primitive is drawn over the ones pushed before it. Quads come before text,
/// so a label always lands over the background behind it.
pub struct DrawList {
    /// Quads with the clip in force when each was pushed.
    quads: Vec<(Quad, Rect)>,
    /// Text runs with the clip in force when each was pushed.
    texts: Vec<(TextRun, Rect)>,
    /// Icons with the clip in force when each was pushed.
    icons: Vec<(IconRun, Rect)>,
    /// The clip stack, never empty; the last entry is in force.
    clips: Vec<Rect>,
}

impl DrawList {
    /// Creates an empty list covering a window of `size`.
    pub fn new(size: Size) -> Self {
        Self {
            quads: Vec::new(),
            texts: Vec::new(),
            icons: Vec::new(),
            clips: vec![Rect::new(Point::default(), size)],
        }
    }

    /// Drops every primitive and resets the clip to a window of `size`.
    pub fn reset(&mut self, size: Size) {
        self.quads.clear();
        self.texts.clear();
        self.icons.clear();
        self.clips.clear();
        self.clips.push(Rect::new(Point::default(), size));
    }

    /// The window the list is drawn for.
    pub fn viewport(&self) -> Rect {
        self.clips.first().copied().unwrap_or_default()
    }

    /// The clip rectangle primitives are currently confined to.
    pub fn clip(&self) -> Rect {
        self.clips.last().copied().unwrap_or_default()
    }

    /// Confines later primitives to `rect` as well as the current clip.
    pub fn push_clip(&mut self, rect: Rect) {
        let clip = self.clip().intersect(rect);
        self.clips.push(clip);
    }

    /// Restores the clip in force before the matching [`Self::push_clip`].
    pub fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    /// Whether anything drawn inside `bounds` would land inside the clip.
    fn is_visible(&self, bounds: Rect) -> bool {
        bounds.overlaps(&self.clip())
    }

    /// Adds a quad, skipping it when it would draw nothing.
    pub fn quad(&mut self, quad: Quad) {
        if quad.is_invisible() || !self.is_visible(quad.bounds) {
            return;
        }
        let clip = self.clip();
        self.quads.push((quad, clip));
    }

    /// Adds a shaped run at `origin` in `color`.
    ///
    /// The run is tested against the clip with a line's height of slack on
    /// every side, because a glyph's ink may reach past its line box: a slanted
    /// letter past the advance, an accent above the ascent.
    pub fn text(&mut self, origin: Point, run: Arc<ShapedRun>, color: Rgba) {
        if color.is_transparent() || run.glyphs.is_empty() {
            return;
        }
        let ink = Rect::new(origin, run.size()).outset(run.height);
        if !self.is_visible(ink) {
            return;
        }
        let clip = self.clip();
        self.texts.push((TextRun { origin, run, color }, clip));
    }

    /// Adds `svg` drawn inside `bounds` in `color`.
    pub fn icon(&mut self, bounds: Rect, svg: Svg, color: Rgba) {
        self.rotated_icon(bounds, svg, color, 0.0);
    }

    /// Adds `svg` rotated around its centre inside `bounds`.
    ///
    /// The clip test allows for the corners a turn swings out past `bounds`.
    pub fn rotated_icon(&mut self, bounds: Rect, svg: Svg, color: Rgba, rotation: f32) {
        if color.is_transparent() || bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            return;
        }
        let turned = bounds.outset(bounds.size.width.max(bounds.size.height) / 2.0);
        if !self.is_visible(turned) {
            return;
        }
        let clip = self.clip();
        self.icons.push((
            IconRun {
                bounds,
                svg,
                color,
                rotation,
            },
            clip,
        ));
    }

    /// The quads to draw, each with its clip rectangle.
    pub(crate) fn quads(&self) -> &[(Quad, Rect)] {
        &self.quads
    }

    /// The icons to draw, each with its clip rectangle.
    pub(crate) fn icons(&self) -> &[(IconRun, Rect)] {
        &self.icons
    }

    /// The text runs to draw, each with its clip rectangle.
    pub(crate) fn texts(&self) -> &[(TextRun, Rect)] {
        &self.texts
    }
}
