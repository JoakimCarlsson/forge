//! A row or a column of panes, each keeping its share of the space, and the sash
//! that moves the division between two of them.
//!
//! A split is the one container whose children are sized by proportion rather
//! than by content: the space is divided between them, and the sash between two
//! neighbours moves that division. It places its children itself, so a split of
//! splits is a tree of panes and nothing else, with no pixel widths to keep in
//! step with the window as it resizes.

use std::sync::Arc;

use fr_math::{Rect, Size};
use fr_render::Quad;

use crate::drag::{DragEvent, DragPhase};
use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::region::RegionAction;
use crate::style::{Axis, Length, Style, Styled};

/// Logical pixels occupied by the visible sash.
const SASH_SIZE: f32 = 1.0;

/// Logical pixels accepting input around the visible sash.
const SASH_HIT_SIZE: f32 = 9.0;

/// The narrowest a pane may be dragged to, until the split is told otherwise.
const DEFAULT_MIN_EXTENT: f32 = 48.0;

/// A divider moved by the pointer: the shares of the two panes either side of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitResize {
    /// Which divider moved: the one after the pane at this index.
    pub divider: usize,
    /// The new share of the pane before the divider.
    pub before: f32,
    /// The new share of the pane after it.
    pub after: f32,
    /// The stage of the drag that moved it.
    pub phase: DragPhase,
}

/// What dragging a divider sends.
type OnResize<M> = Arc<dyn Fn(SplitResize) -> M>;

/// A container dividing the space it is given between its children.
pub struct Split<M> {
    /// The axis the children are divided along.
    axis: Axis,
    /// The children, in order along that axis.
    children: Vec<Box<dyn Element<M>>>,
    /// Each child's share of the space, in the same order.
    shares: Vec<f32>,
    /// What dragging a divider sends, when the caller listens for it.
    on_resize: Option<OnResize<M>>,
    /// The least extent a pane is dragged down to, in logical pixels.
    min_extent: f32,
    /// How the split is sized within its parent.
    style: Style,
}

/// An empty split dividing its space along `axis`.
pub fn split<M>(axis: Axis) -> Split<M> {
    Split {
        axis,
        children: Vec::new(),
        shares: Vec::new(),
        on_resize: None,
        min_extent: DEFAULT_MIN_EXTENT,
        style: Style::default(),
    }
    .w_full()
    .h_full()
}

impl<M> Split<M> {
    /// Appends one child taking `share` of the space.
    ///
    /// Shares are weights rather than fractions: a child is given its share of
    /// their total, so a split whose children read 1 and 1 is the same as one
    /// whose children read 0.5 and 0.5.
    pub fn child(mut self, share: f32, child: impl IntoElement<M>) -> Self {
        self.children.push(child.into_element());
        self.shares.push(share.max(0.0));
        self
    }

    /// Returns this split reporting divider drags through `on_resize`.
    pub fn on_resize(mut self, on_resize: impl Fn(SplitResize) -> M + 'static) -> Self {
        self.on_resize = Some(Arc::new(on_resize));
        self
    }

    /// Returns this split with panes that cannot be dragged narrower than `pixels`.
    pub fn min_extent(mut self, pixels: f32) -> Self {
        self.min_extent = pixels.max(0.0);
        self
    }

    /// The extent left for the panes once the dividers have taken theirs.
    fn pane_extent(&self, bounds: Rect) -> f32 {
        let dividers = self.children.len().saturating_sub(1) as f32 * SASH_SIZE;
        (self.axis.main_of(bounds.size) - dividers).max(0.0)
    }

    /// Each child's extent along the axis, in the order they are painted.
    fn extents(&self, bounds: Rect) -> Vec<f32> {
        let extent = self.pane_extent(bounds);
        let total: f32 = self.shares.iter().sum();
        if total <= 0.0 {
            let each = extent / self.children.len().max(1) as f32;
            return vec![each; self.children.len()];
        }
        self.shares
            .iter()
            .map(|share| extent * share / total)
            .collect()
    }

    /// The rectangle a child of `extent` occupies at `offset` along the axis.
    fn slot(&self, bounds: Rect, offset: f32, extent: f32) -> Rect {
        match self.axis {
            Axis::Horizontal => Rect::from_xywh(
                bounds.left() + offset,
                bounds.top(),
                extent,
                bounds.size.height,
            ),
            Axis::Vertical => Rect::from_xywh(
                bounds.left(),
                bounds.top() + offset,
                bounds.size.width,
                extent,
            ),
        }
    }

    /// The handler of the sash after pane `divider`, bound to the shares and
    /// extent this frame is painted with.
    fn resize_handler(&self, divider: usize, bounds: Rect) -> Option<crate::region::DragHandler<M>>
    where
        M: 'static,
    {
        let on_resize = self.on_resize.clone()?;
        let shares = self.shares.clone();
        let extent = self.pane_extent(bounds).max(1.0);
        let min_extent = self.min_extent;
        let axis = self.axis;
        Some(Arc::new(move |event: DragEvent| {
            let travelled = match event.phase {
                DragPhase::Cancelled => 0.0,
                _ => event.total_along(axis),
            };
            let (before, after) = moved_shares(&shares, divider, travelled, extent, min_extent);
            on_resize(SplitResize {
                divider,
                before,
                after,
                phase: event.phase,
            })
        }))
    }
}

/// The shares of the panes either side of divider `divider` after it travelled
/// `pixels`, the pair keeping the total they had and neither shrinking under
/// `min_extent` pixels.
fn moved_shares(
    shares: &[f32],
    divider: usize,
    pixels: f32,
    extent: f32,
    min_extent: f32,
) -> (f32, f32) {
    let before = shares.get(divider).copied().unwrap_or(0.0);
    let after = shares.get(divider + 1).copied().unwrap_or(0.0);
    let total: f32 = shares.iter().sum();
    let pair = before + after;
    let per_pixel = total / extent;
    let floor = (min_extent * per_pixel).min(pair / 2.0);
    let moved = (before + pixels * per_pixel).clamp(floor, pair - floor);
    (moved, pair - moved)
}

impl<M> Styled for Split<M> {
    /// How the split is sized within its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for Split<M> {
    /// How the split is sized within its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes everything it is offered; a split is as large as its region.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            _ => available.width,
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            _ => available.height,
        };
        Size::new(width, height)
    }

    /// Places every child in its share of the bounds, dividers between them.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let extents = self.extents(bounds);
        let axis = self.axis;
        let last = self.children.len().saturating_sub(1);
        let mut offset = 0.0;
        let mut slots = Vec::with_capacity(self.children.len());
        for extent in &extents {
            slots.push(self.slot(bounds, offset, *extent));
            offset += extent + SASH_SIZE;
        }

        let handlers: Vec<_> = (0..last)
            .map(|index| self.resize_handler(index, bounds))
            .collect();
        for (index, (child, slot)) in self.children.iter_mut().zip(&slots).enumerate() {
            child.paint(*slot, cx);
            if index == last {
                continue;
            }
            let line = divider_slot(axis, bounds, *slot);
            paint_divider(cx, line, axis, handlers[index].clone());
        }
    }
}

/// The divider drawn straight after the child occupying `slot`.
fn divider_slot(axis: Axis, bounds: Rect, slot: Rect) -> Rect {
    match axis {
        Axis::Horizontal => {
            Rect::from_xywh(slot.right(), bounds.top(), SASH_SIZE, bounds.size.height)
        }
        Axis::Vertical => {
            Rect::from_xywh(bounds.left(), slot.bottom(), bounds.size.width, SASH_SIZE)
        }
    }
}

/// The wider invisible pointer target centred on the divider filling `bounds`.
fn hit_bounds(bounds: Rect, axis: Axis) -> Rect {
    match axis {
        Axis::Horizontal => Rect::from_xywh(
            bounds.left() - (SASH_HIT_SIZE - bounds.size.width) / 2.0,
            bounds.top(),
            SASH_HIT_SIZE,
            bounds.size.height,
        ),
        Axis::Vertical => Rect::from_xywh(
            bounds.left(),
            bounds.top() - (SASH_HIT_SIZE - bounds.size.height) / 2.0,
            bounds.size.width,
            SASH_HIT_SIZE,
        ),
    }
}

/// Paints the divider filling `bounds` and registers the drag that moves it.
///
/// Every edge the pointer moves is drawn and hit the same way, whether it
/// divides two panes of a split or stands alone as a [`Sash`], so the line, the
/// colour it lights up in and the forgiving target around it live here.
pub(crate) fn paint_divider<M>(
    cx: &mut PaintContext<'_, '_, M>,
    bounds: Rect,
    axis: Axis,
    handler: Option<crate::region::DragHandler<M>>,
) {
    let hit = hit_bounds(bounds, axis);
    let interaction = match handler {
        Some(handler) => cx.region(hit, RegionAction::inert().drag(handler)),
        None => Default::default(),
    };
    let color = if interaction.pressed || interaction.hovered {
        cx.theme().colors.border_focused
    } else {
        cx.theme().colors.border
    };
    cx.quad(Quad::filled(bounds, color));
}

/// A thin divider with a forgiving hit area that captures pointer drags.
pub struct Sash<M> {
    /// The dimension changed by dragging the sash.
    axis: Axis,
    /// Builds the caller's message for each drag event.
    on_drag: Arc<dyn Fn(DragEvent) -> M>,
}

/// A sash changing the extent along `axis`, sending `on_drag` for its drag.
pub fn sash<M>(axis: Axis, on_drag: impl Fn(DragEvent) -> M + 'static) -> Sash<M> {
    Sash {
        axis,
        on_drag: Arc::new(on_drag),
    }
}

impl<M: 'static> Element<M> for Sash<M> {
    /// Sizes the sash to a hairline across the other axis.
    fn layout_style(&self) -> Style {
        let mut style = Style::default();
        match self.axis {
            Axis::Horizontal => {
                style.width = Length::Px(SASH_SIZE);
                style.height = Length::Full;
            }
            Axis::Vertical => {
                style.width = Length::Full;
                style.height = Length::Px(SASH_SIZE);
            }
        }
        style
    }

    /// Takes the offered cross-axis space and the line's width on the drag axis.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        match self.axis {
            Axis::Horizontal => Size::new(SASH_SIZE, available.height),
            Axis::Vertical => Size::new(available.width, SASH_SIZE),
        }
    }

    /// Registers the hit area and paints the divider.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        paint_divider(cx, bounds, self.axis, Some(self.on_drag.clone()));
    }
}
