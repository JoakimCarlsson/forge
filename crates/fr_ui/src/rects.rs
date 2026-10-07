//! Where elements were painted, left for the caller to read.
//!
//! Layout happens while the frame is drawn, so nothing outside the tree knows
//! where anything came out until it has been: a gesture that ends somewhere
//! else entirely, a tab carried across the window to another pane, is answered
//! by the caller, not by the element it started in. This is how the caller is
//! told. An element paints and writes its bounds into a handle it shares with
//! the caller, who reads them back after the frame.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use fr_math::{Point, Rect, Size};

use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::style::Style;

/// Where one element was last painted, shared with whoever wants to know.
pub type Bounds = Rc<Cell<Rect>>;

/// The bounds of many elements, by name, as of the last frame that painted them.
///
/// A handle is cheap to clone and every clone shares the same table. Entries
/// are replaced as elements paint and are not removed on their own: call
/// [`Rects::clear`] before building a frame to drop what no longer exists.
#[derive(Clone, Debug, Default)]
pub struct Rects {
    /// The recorded bounds, in key order.
    table: Rc<RefCell<BTreeMap<String, Rect>>>,
}

impl Rects {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets every recorded bounds.
    pub fn clear(&self) {
        self.table.borrow_mut().clear();
    }

    /// Records that `key` was painted at `bounds`.
    pub fn set(&self, key: impl Into<String>, bounds: Rect) {
        self.table.borrow_mut().insert(key.into(), bounds);
    }

    /// Where `key` was last painted.
    pub fn get(&self, key: &str) -> Option<Rect> {
        self.table.borrow().get(key).copied()
    }

    /// The key painted at `point`, the last in key order when several overlap.
    pub fn key_at(&self, point: Point) -> Option<String> {
        self.table
            .borrow()
            .iter()
            .rev()
            .find(|(_, bounds)| bounds.contains(point))
            .map(|(key, _)| key.clone())
    }

    /// Every recorded key and its bounds, in key order.
    pub fn entries(&self) -> Vec<(String, Rect)> {
        self.table
            .borrow()
            .iter()
            .map(|(key, bounds)| (key.clone(), *bounds))
            .collect()
    }
}

/// An element that reports its own bounds as it paints.
pub struct Measured<M> {
    /// Where the child was painted, as of the last frame.
    bounds: Bounds,
    /// What is drawn there.
    child: Box<dyn Element<M>>,
}

/// `child`, leaving where it was painted in `bounds`.
pub fn measured<M>(bounds: Bounds, child: impl IntoElement<M>) -> Measured<M> {
    Measured {
        bounds,
        child: child.into_element(),
    }
}

impl<M> Element<M> for Measured<M> {
    /// The child's own style; measuring changes nothing about layout.
    fn layout_style(&self) -> Style {
        self.child.layout_style()
    }

    /// Whatever the child measures to.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.child.measure(available, cx)
    }

    /// Paints the child and writes down where that was.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.bounds.set(bounds);
        self.child.paint(bounds, cx);
    }
}

/// An element that records its bounds under a key in a [`Rects`] as it paints.
pub struct Placed<M> {
    /// The table the bounds are written to.
    rects: Rects,
    /// The key they are written under.
    key: String,
    /// What is drawn there.
    child: Box<dyn Element<M>>,
}

/// `child`, leaving where it was painted in `rects` under `key`.
pub fn placed<M>(rects: &Rects, key: impl Into<String>, child: impl IntoElement<M>) -> Placed<M> {
    Placed {
        rects: rects.clone(),
        key: key.into(),
        child: child.into_element(),
    }
}

impl<M> Element<M> for Placed<M> {
    /// The child's own style; recording changes nothing about layout.
    fn layout_style(&self) -> Style {
        self.child.layout_style()
    }

    /// Whatever the child measures to.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.child.measure(available, cx)
    }

    /// Paints the child and writes down where that was.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.rects.set(self.key.clone(), bounds);
        self.child.paint(bounds, cx);
    }
}
