//! The layered recording of one frame's primitives, and its replay into a draw list.
//!
//! [`fr_render::DrawList`] draws every quad of a frame, then every text run, then
//! every icon, so a popup painted late would still sit under the glyphs of the
//! screen it covers. The canvas is what makes layers work on top of it: elements
//! paint into the canvas with a layer number, and [`Canvas::flush`] replays the
//! primitives layer by layer, drawing each glyph run and icon of a lower layer
//! only through the part of its clip that no opaque quad of a higher layer covers.

use std::sync::Arc;

use fr_color::Rgba;
use fr_math::{Point, Rect};
use fr_render::{DrawList, Quad, ShapedRun, Svg};

/// The least opacity at which a quad hides what is drawn beneath it.
const OPAQUE: f32 = 0.85;

/// One recorded primitive.
enum Primitive {
    /// A rounded, optionally bordered rectangle.
    Quad(Quad),
    /// A shaped run of text.
    Text {
        /// Where the line box starts.
        origin: Point,
        /// The glyphs.
        run: Arc<ShapedRun>,
        /// The tint.
        color: Rgba,
    },
    /// A tinted vector icon.
    Icon {
        /// The square the artwork is drawn inside.
        bounds: Rect,
        /// The artwork.
        svg: Svg,
        /// The tint.
        color: Rgba,
        /// Clockwise rotation around the centre, in radians.
        rotation: f32,
    },
}

impl Primitive {
    /// The rectangle the primitive's ink can reach, which a clip is tested against.
    fn ink(&self) -> Rect {
        match self {
            Self::Quad(quad) => quad.rotated_bounds(),
            Self::Text { origin, run, .. } => Rect::new(*origin, run.size()).outset(run.height),
            Self::Icon {
                bounds, rotation, ..
            } => {
                if *rotation == 0.0 {
                    *bounds
                } else {
                    bounds.outset(bounds.size.width.max(bounds.size.height) / 2.0)
                }
            }
        }
    }
}

/// A primitive with the layer and clip it was painted under.
struct Entry {
    /// The layer, higher drawing over lower.
    layer: u32,
    /// The clip in force when it was painted.
    clip: Rect,
    /// What was painted.
    primitive: Primitive,
}

/// A layer entered by [`Canvas::push_layer`], and where to cut the clips back to.
struct LayerFrame {
    /// The layer that was in force before.
    outer: u32,
    /// How many clips were stacked before the layer began.
    clips: usize,
}

/// A frame's primitives, recorded with their layers and clips.
pub(crate) struct Canvas {
    /// Everything painted so far, in paint order.
    entries: Vec<Entry>,
    /// The clip stack, never empty; the last entry is in force.
    clips: Vec<Rect>,
    /// The layer being painted.
    layer: u32,
    /// The highest layer handed out so far.
    last_layer: u32,
    /// The layers entered and not yet left.
    frames: Vec<LayerFrame>,
    /// The window the frame is for.
    viewport: Rect,
}

impl Canvas {
    /// An empty canvas for a window of `viewport`.
    pub(crate) fn new(viewport: Rect) -> Self {
        Self {
            entries: Vec::new(),
            clips: vec![viewport],
            layer: 0,
            last_layer: 0,
            frames: Vec::new(),
            viewport,
        }
    }

    /// Forgets everything recorded and starts over for a window of `viewport`.
    pub(crate) fn reset(&mut self, viewport: Rect) {
        self.entries.clear();
        self.clips.clear();
        self.clips.push(viewport);
        self.frames.clear();
        self.layer = 0;
        self.last_layer = 0;
        self.viewport = viewport;
    }

    /// The window the frame is for.
    pub(crate) fn viewport(&self) -> Rect {
        self.viewport
    }

    /// The clip primitives are confined to right now.
    pub(crate) fn clip(&self) -> Rect {
        self.clips.last().copied().unwrap_or(self.viewport)
    }

    /// The layer being painted.
    pub(crate) fn layer(&self) -> u32 {
        self.layer
    }

    /// Confines later primitives to `rect` as well as the current clip.
    pub(crate) fn push_clip(&mut self, rect: Rect) {
        let clip = self.clip().intersect(rect);
        self.clips.push(clip);
    }

    /// Restores the clip in force before the matching [`Self::push_clip`].
    pub(crate) fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    /// Starts a layer above every layer so far, clipped to the window alone.
    pub(crate) fn push_layer(&mut self) {
        self.frames.push(LayerFrame {
            outer: self.layer,
            clips: self.clips.len(),
        });
        self.last_layer += 1;
        self.layer = self.last_layer;
        self.clips.push(self.viewport);
    }

    /// Returns to the layer and clip in force before the matching [`Self::push_layer`].
    pub(crate) fn pop_layer(&mut self) {
        if let Some(frame) = self.frames.pop() {
            self.layer = frame.outer;
            self.clips.truncate(frame.clips.max(1));
        }
    }

    /// Records `primitive` under the current layer and clip.
    fn record(&mut self, primitive: Primitive) {
        let clip = self.clip();
        if clip.size.width <= 0.0 || clip.size.height <= 0.0 {
            return;
        }
        self.entries.push(Entry {
            layer: self.layer,
            clip,
            primitive,
        });
    }

    /// Records a quad.
    pub(crate) fn quad(&mut self, quad: Quad) {
        if quad.is_invisible() {
            return;
        }
        self.record(Primitive::Quad(quad));
    }

    /// Records a shaped run at `origin` in `color`.
    pub(crate) fn text(&mut self, origin: Point, run: Arc<ShapedRun>, color: Rgba) {
        if color.is_transparent() {
            return;
        }
        self.record(Primitive::Text { origin, run, color });
    }

    /// Records `svg` in `bounds`, tinted `color` and turned `rotation` radians.
    pub(crate) fn icon(&mut self, bounds: Rect, svg: Svg, color: Rgba, rotation: f32) {
        if color.is_transparent() {
            return;
        }
        self.record(Primitive::Icon {
            bounds,
            svg,
            color,
            rotation,
        });
    }

    /// The opaque quads of each layer above the lowest, as a layer and a clipped area.
    fn occluders(&self) -> Vec<(u32, Rect)> {
        self.entries
            .iter()
            .filter(|entry| entry.layer > 0)
            .filter_map(|entry| match &entry.primitive {
                Primitive::Quad(quad) if quad.background.a >= OPAQUE && quad.rotation == 0.0 => {
                    Some((entry.layer, quad.bounds.intersect(entry.clip)))
                }
                _ => None,
            })
            .filter(|(_, area)| area.size.width > 0.0 && area.size.height > 0.0)
            .collect()
    }

    /// Replays the recording into `list`, layer by layer.
    pub(crate) fn flush(&mut self, list: &mut DrawList) {
        self.entries.sort_by_key(|entry| entry.layer);
        let occluders = self.occluders();
        for entry in &self.entries {
            let covering: Vec<Rect> = occluders
                .iter()
                .filter(|(layer, _)| *layer > entry.layer)
                .map(|(_, area)| *area)
                .collect();
            let ink = entry.primitive.ink().intersect(entry.clip);
            let covering: Vec<Rect> = covering
                .into_iter()
                .filter(|area| area.overlaps(&ink))
                .collect();
            let is_quad = matches!(entry.primitive, Primitive::Quad(_));
            if is_quad || covering.is_empty() {
                emit(list, entry.clip, &entry.primitive);
                continue;
            }
            for piece in visible_pieces(entry.clip, &covering) {
                emit(list, piece, &entry.primitive);
            }
        }
    }
}

/// Draws `primitive` into `list` through `clip` alone.
fn emit(list: &mut DrawList, clip: Rect, primitive: &Primitive) {
    list.push_clip(clip);
    match primitive {
        Primitive::Quad(quad) => list.quad(*quad),
        Primitive::Text { origin, run, color } => list.text(*origin, run.clone(), *color),
        Primitive::Icon {
            bounds,
            svg,
            color,
            rotation,
        } => list.rotated_icon(*bounds, *svg, *color, *rotation),
    }
    list.pop_clip();
}

/// The parts of `clip` that none of `covering` hides, as disjoint rectangles.
fn visible_pieces(clip: Rect, covering: &[Rect]) -> Vec<Rect> {
    let mut pieces = vec![clip];
    for cover in covering {
        pieces = pieces
            .into_iter()
            .flat_map(|piece| subtract(piece, *cover))
            .collect();
    }
    pieces
}

/// `piece` with `cover` cut out of it: up to four disjoint rectangles.
fn subtract(piece: Rect, cover: Rect) -> Vec<Rect> {
    if !piece.overlaps(&cover) {
        return vec![piece];
    }
    let inner = piece.intersect(cover);
    let mut parts = Vec::with_capacity(4);
    let mut push = |rect: Rect| {
        if rect.size.width > 0.0 && rect.size.height > 0.0 {
            parts.push(rect);
        }
    };
    push(Rect::from_xywh(
        piece.left(),
        piece.top(),
        piece.size.width,
        inner.top() - piece.top(),
    ));
    push(Rect::from_xywh(
        piece.left(),
        inner.bottom(),
        piece.size.width,
        piece.bottom() - inner.bottom(),
    ));
    push(Rect::from_xywh(
        piece.left(),
        inner.top(),
        inner.left() - piece.left(),
        inner.size.height,
    ));
    push(Rect::from_xywh(
        inner.right(),
        inner.top(),
        piece.right() - inner.right(),
        inner.size.height,
    ));
    parts
}
