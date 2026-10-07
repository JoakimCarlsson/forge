//! The regions a frame leaves behind, and what each one does with the pointer.

use std::sync::Arc;

use fr_input::ScrollDelta;
use fr_math::{Point, Rect};

use crate::drag::DragEvent;

/// Builds the caller's message for each event of a captured drag.
pub type DragHandler<M> = Arc<dyn Fn(DragEvent) -> M>;

/// Builds the caller's message for a press, given where it landed.
pub type PressHandler<M> = Arc<dyn Fn(Point) -> M>;

/// Builds the caller's message for a turn of the wheel.
pub type ScrollHandler<M> = Arc<dyn Fn(ScrollDelta) -> M>;

/// A painted region that answers to the pointer and to the keyboard.
///
/// Regions are recorded in paint order, which is therefore also tab order. The
/// topmost region under a point is the one on the highest layer and, within a
/// layer, the one painted last.
pub struct Region<M> {
    /// Where the region is, cut down to the clip it was painted under.
    pub bounds: Rect,
    /// The layer it was painted on; a higher layer is over a lower one.
    pub layer: u32,
    /// What it does with pointer input.
    pub action: RegionAction<M>,
}

/// What a region does with the pointer; a region with none of it is inert.
///
/// An inert region is still a region: the pointer is over it, and it keeps
/// whatever lies under it from being pressed.
pub struct RegionAction<M> {
    /// Sent when a press and release both land in the region.
    pub click: Option<M>,
    /// Sent instead of `click` when a click follows another quickly enough.
    pub double_click: Option<M>,
    /// Sent when the secondary button goes down on the region.
    pub secondary: Option<M>,
    /// Builds the message of a press, the moment the primary button goes down.
    pub press: Option<PressHandler<M>>,
    /// Builds the messages of a drag that starts on the region.
    pub drag: Option<DragHandler<M>>,
    /// Builds the message of a turn of the wheel over the region.
    pub scroll: Option<ScrollHandler<M>>,
}

impl<M> RegionAction<M> {
    /// An action that does nothing, which still blocks what is under it.
    pub fn inert() -> Self {
        Self {
            click: None,
            double_click: None,
            secondary: None,
            press: None,
            drag: None,
            scroll: None,
        }
    }

    /// Returns this action sending `message` when the region is clicked.
    pub fn click(mut self, message: M) -> Self {
        self.click = Some(message);
        self
    }

    /// Returns this action sending `message` when the region is clicked twice.
    pub fn double_click(mut self, message: M) -> Self {
        self.double_click = Some(message);
        self
    }

    /// Returns this action sending `message` when the secondary button goes down.
    pub fn secondary(mut self, message: M) -> Self {
        self.secondary = Some(message);
        self
    }

    /// Returns this action building a message from where the primary button went down.
    pub fn press(mut self, handler: PressHandler<M>) -> Self {
        self.press = Some(handler);
        self
    }

    /// Returns this action capturing drags that start on the region.
    pub fn drag(mut self, handler: DragHandler<M>) -> Self {
        self.drag = Some(handler);
        self
    }

    /// Returns this action turning the wheel over the region into a message.
    pub fn scroll(mut self, handler: ScrollHandler<M>) -> Self {
        self.scroll = Some(handler);
        self
    }

    /// Whether the action answers to anything at all, beyond blocking.
    pub fn is_active(&self) -> bool {
        self.click.is_some()
            || self.double_click.is_some()
            || self.secondary.is_some()
            || self.press.is_some()
            || self.drag.is_some()
            || self.scroll.is_some()
    }

    /// Whether the region can be reached by tabbing and activated by the keyboard.
    pub fn is_focusable(&self) -> bool {
        self.click.is_some()
    }
}

impl<M: Clone> Clone for RegionAction<M> {
    /// Copies the messages and shares the handlers.
    fn clone(&self) -> Self {
        Self {
            click: self.click.clone(),
            double_click: self.double_click.clone(),
            secondary: self.secondary.clone(),
            press: self.press.clone(),
            drag: self.drag.clone(),
            scroll: self.scroll.clone(),
        }
    }
}

impl<M> Default for RegionAction<M> {
    /// An inert action.
    fn default() -> Self {
        Self::inert()
    }
}
