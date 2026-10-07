//! The events of a captured pointer drag.

use fr_input::Modifiers;
use fr_math::Point;

use crate::style::Axis;

/// The stage of a captured drag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DragPhase {
    /// The pointer travelled far enough from where it went down to be a drag.
    Began,
    /// The captured pointer moved.
    Moved,
    /// The captured pointer was released.
    Ended,
    /// The drag was abandoned, by escape or by the window losing the pointer.
    Cancelled,
}

/// One event of a drag captured by the region the press landed on.
///
/// The drag is captured for the whole press: its events keep coming when the
/// pointer leaves the region, and when it leaves the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragEvent {
    /// The stage of the drag.
    pub phase: DragPhase,
    /// Where the pointer went down.
    pub start: Point,
    /// Where the pointer is now.
    pub position: Point,
    /// How far the pointer moved since the previous event of this drag.
    pub delta: Point,
    /// The modifier keys held at this event.
    pub modifiers: Modifiers,
}

impl DragEvent {
    /// How far the pointer is from where it went down.
    pub fn total(self) -> Point {
        Point::new(
            self.position.x - self.start.x,
            self.position.y - self.start.y,
        )
    }

    /// How far the pointer is from where it went down, along `axis`.
    pub fn total_along(self, axis: Axis) -> f32 {
        along(self.total(), axis)
    }

    /// How far the pointer moved since the previous event, along `axis`.
    pub fn delta_along(self, axis: Axis) -> f32 {
        along(self.delta, axis)
    }

    /// Whether this event ends the drag, by release or by cancellation.
    pub fn is_final(self) -> bool {
        matches!(self.phase, DragPhase::Ended | DragPhase::Cancelled)
    }
}

/// The component of `point` along `axis`.
fn along(point: Point, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}
