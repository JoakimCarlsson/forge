//! Something drawn over the screen rather than laid out inside it.
//!
//! An overlay takes no room: it is measured as nothing, so the screen around it
//! is laid out as though it were not there, and then painted at a point of its
//! own, on a layer of its own. The layer is what makes it an overlay rather
//! than a late sibling: it is drawn over the text of the screen it covers, and
//! the regions it registers are over the regions below, which keeps a click on
//! the overlay from reaching what it hides.
//!
//! An overlay that is a popup also covers the window with an invisible region
//! behind it. A press on that region, with either button, sends the message the
//! caller gave to dismiss the popup, and goes no further.

use std::sync::Arc;

use fr_math::{Point, Rect, Size};

use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::region::RegionAction;
use crate::style::Style;

/// How close to the edge of the window an overlay may come.
const MARGIN: f32 = 4.0;

/// Registers the region over the whole window that sends `dismiss` on a press.
fn paint_backdrop<M: Clone + 'static>(cx: &mut PaintContext<'_, '_, M>, dismiss: &M) {
    let on_primary = dismiss.clone();
    let action = RegionAction::inert()
        .press(Arc::new(move |_| on_primary.clone()))
        .secondary(dismiss.clone());
    let viewport = cx.viewport();
    cx.region(viewport, action);
}

/// Where a panel of `size` goes with its top left corner wanted at `x`, `y`,
/// moved to stay inside `window`.
fn keep_inside(window: Rect, x: f32, y: f32, size: Size) -> Rect {
    let left = x
        .min(window.right() - size.width - MARGIN)
        .max(window.left() + MARGIN);
    let top = y
        .min(window.bottom() - size.height - MARGIN)
        .max(window.top() + MARGIN);
    Rect::from_xywh(left, top, size.width, size.height)
}

/// Paints `panel` into `rect` on a layer of its own, behind it a backdrop that
/// sends `dismiss` when there is one.
fn paint_layered<M: Clone + 'static>(
    cx: &mut PaintContext<'_, '_, M>,
    panel: &mut dyn Element<M>,
    rect: Rect,
    dismiss: Option<&M>,
) {
    cx.push_layer();
    if let Some(dismiss) = dismiss {
        paint_backdrop(cx, dismiss);
    }
    panel.paint(rect, cx);
    cx.pop_layer();
}

/// One element painted at a point of the window's own choosing.
pub struct Overlay<M> {
    /// Where the overlay would like its top left corner to be.
    origin: Point,
    /// What is drawn there.
    child: Box<dyn Element<M>>,
    /// Whether `origin` is the bottom left corner instead, so the overlay
    /// grows upward from it.
    rises: bool,
    /// What a press outside the overlay sends, making it a popup.
    dismiss: Option<M>,
}

/// `child`, painted at `origin` over whatever was painted before it.
pub fn overlay<M>(origin: Point, child: impl IntoElement<M>) -> Overlay<M> {
    Overlay {
        origin,
        child: child.into_element(),
        rises: false,
        dismiss: None,
    }
}

/// `child`, painted with its bottom left corner at `origin`: the overlay a
/// control along the bottom of the window opens, which belongs above it.
pub fn overlay_above<M>(origin: Point, child: impl IntoElement<M>) -> Overlay<M> {
    Overlay {
        origin,
        child: child.into_element(),
        rises: true,
        dismiss: None,
    }
}

/// `panel`, painted at `origin` as a popup: a press anywhere outside it sends
/// `dismiss`, which is what closes a context menu.
pub fn popup_at<M>(origin: Point, dismiss: M, panel: impl IntoElement<M>) -> Overlay<M> {
    overlay(origin, panel).on_dismiss(dismiss)
}

impl<M> Overlay<M> {
    /// Returns this overlay a popup, sending `message` for a press outside it.
    pub fn on_dismiss(mut self, message: M) -> Self {
        self.dismiss = Some(message);
        self
    }
}

impl<M: Clone + 'static> Element<M> for Overlay<M> {
    /// An overlay is laid out as nothing; it is placed, not stacked.
    fn layout_style(&self) -> Style {
        Style::default()
    }

    /// Takes no room from the screen it is drawn over.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        Size::zero()
    }

    /// Paints the child at its point, moved to keep it inside the window.
    fn paint(&mut self, _bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let window = cx.viewport();
        let size = self.child.measure(window.size, &mut cx.layout);
        let top = match self.rises {
            true => self.origin.y - size.height,
            false => self.origin.y,
        };
        let rect = keep_inside(window, self.origin.x, top, size);
        paint_layered(cx, &mut self.child, rect, self.dismiss.as_ref());
    }
}

/// One element painted beside another, on a layer of its own.
///
/// This is the overlay that has somewhere to be: a submenu belongs against the
/// row that opened it, and the row only knows where it is once it has been
/// laid out. The panel takes no room either way — the screen is laid out as
/// though only the anchor were there.
pub struct Beside<M> {
    /// The element the panel is placed against.
    anchor: Box<dyn Element<M>>,
    /// What is drawn beside it.
    panel: Box<dyn Element<M>>,
}

/// `panel`, painted against the right edge of `anchor`.
pub fn beside<M>(anchor: impl IntoElement<M>, panel: impl IntoElement<M>) -> Beside<M> {
    Beside {
        anchor: anchor.into_element(),
        panel: panel.into_element(),
    }
}

impl<M: Clone + 'static> Element<M> for Beside<M> {
    /// Lays out as the anchor does; the panel is placed, not stacked.
    fn layout_style(&self) -> Style {
        self.anchor.layout_style()
    }

    /// Asks for what the anchor asks for, the panel taking no room.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.anchor.measure(available, cx)
    }

    /// Paints the anchor where it belongs, then the panel against its edge.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.anchor.paint(bounds, cx);
        let window = cx.viewport();
        let size = self.panel.measure(window.size, &mut cx.layout);
        let rect = keep_inside(window, bounds.right(), bounds.top(), size);
        paint_layered(cx, &mut self.panel, rect, None);
    }
}

/// One element painted on top of another, on a layer of its own.
///
/// This is the overlay a control along the bottom of a pane opens over the
/// content above it: a list of completions belongs against the box being typed
/// in, as wide as it, and must not push what is above out of the way. The panel
/// takes no room — the screen is laid out as though only the anchor were there.
pub struct Above<M> {
    /// The element the panel is placed on top of.
    anchor: Box<dyn Element<M>>,
    /// What is drawn above it.
    panel: Box<dyn Element<M>>,
}

/// `panel`, painted as wide as `anchor` with its bottom against the anchor's
/// top edge.
pub fn above<M>(anchor: impl IntoElement<M>, panel: impl IntoElement<M>) -> Above<M> {
    Above {
        anchor: anchor.into_element(),
        panel: panel.into_element(),
    }
}

impl<M: Clone + 'static> Element<M> for Above<M> {
    /// Lays out as the anchor does; the panel is placed, not stacked.
    fn layout_style(&self) -> Style {
        self.anchor.layout_style()
    }

    /// Asks for what the anchor asks for, the panel taking no room.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.anchor.measure(available, cx)
    }

    /// Paints the anchor where it belongs, then the panel on top of it.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.anchor.paint(bounds, cx);
        let window = cx.viewport();
        let size = self.panel.measure(
            Size::new(bounds.size.width, window.size.height),
            &mut cx.layout,
        );
        let y = (bounds.top() - size.height).max(window.top() + MARGIN);
        let rect = Rect::from_xywh(bounds.left(), y, bounds.size.width, size.height);
        paint_layered(cx, &mut self.panel, rect, None);
    }
}

/// One element painted below another as a popup, flipping above it when there is
/// no room below.
///
/// This is what a menu bar entry, a combo box and a colour swatch open: the
/// panel hangs from the anchor, is at least as wide as it, and a press anywhere
/// else sends the caller's dismiss message.
pub struct Dropdown<M> {
    /// The element the panel hangs from.
    anchor: Box<dyn Element<M>>,
    /// What is drawn below it.
    panel: Box<dyn Element<M>>,
    /// What a press outside the panel sends.
    dismiss: Option<M>,
    /// Whether the panel is at least as wide as the anchor.
    match_width: bool,
}

/// `panel`, painted below `anchor` as a popup.
pub fn dropdown<M>(anchor: impl IntoElement<M>, panel: impl IntoElement<M>) -> Dropdown<M> {
    Dropdown {
        anchor: anchor.into_element(),
        panel: panel.into_element(),
        dismiss: None,
        match_width: false,
    }
}

impl<M> Dropdown<M> {
    /// Returns this dropdown sending `message` for a press outside the panel.
    pub fn on_dismiss(mut self, message: M) -> Self {
        self.dismiss = Some(message);
        self
    }

    /// Returns this dropdown with a panel at least as wide as its anchor.
    pub fn match_width(mut self) -> Self {
        self.match_width = true;
        self
    }
}

impl<M: Clone + 'static> Element<M> for Dropdown<M> {
    /// Lays out as the anchor does; the panel is placed, not stacked.
    fn layout_style(&self) -> Style {
        self.anchor.layout_style()
    }

    /// Asks for what the anchor asks for, the panel taking no room.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.anchor.measure(available, cx)
    }

    /// Paints the anchor where it belongs, then the panel hanging from it.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.anchor.paint(bounds, cx);
        let window = cx.viewport();
        let offer_width = match self.match_width {
            true => bounds.size.width,
            false => window.size.width,
        };
        let mut size = self
            .panel
            .measure(Size::new(offer_width, window.size.height), &mut cx.layout);
        if self.match_width {
            size.width = size.width.max(bounds.size.width);
        }
        let below = bounds.bottom() + 2.0;
        let fits = below + size.height + MARGIN <= window.bottom();
        let top = match fits {
            true => below,
            false => bounds.top() - size.height - 2.0,
        };
        let rect = keep_inside(window, bounds.left(), top, size);
        paint_layered(cx, &mut self.panel, rect, self.dismiss.as_ref());
    }
}
