//! The tooltip: a small note shown once the pointer has rested on an element.

use std::time::{Duration, Instant};

use fr_math::{Point, Rect};
use fr_render::Quad;

use crate::element::PaintContext;

/// How long the pointer rests on an element before its tooltip shows.
pub(crate) const DELAY: Duration = Duration::from_millis(500);

/// Space between the tooltip's text and its edge.
const PADDING: f32 = 6.0;

/// Space between the tooltip and the element it explains.
const GAP: f32 = 4.0;

/// A tooltip the pointer is resting on, and since when.
pub(crate) struct HeldTooltip {
    /// Where the element asking for it was painted.
    pub(crate) bounds: Rect,
    /// What it says.
    pub(crate) text: String,
    /// When the pointer came to rest on it.
    pub(crate) since: Instant,
}

impl HeldTooltip {
    /// Keeps the rest time of `held` while `asked` is the same tooltip, or starts it anew.
    pub(crate) fn hold(
        held: Option<Self>,
        asked: Option<(Rect, String)>,
        now: Instant,
    ) -> Option<Self> {
        asked.map(|(bounds, text)| {
            let since = held
                .filter(|held| held.bounds == bounds && held.text == text)
                .map_or(now, |held| held.since);
            Self {
                bounds,
                text,
                since,
            }
        })
    }

    /// Whether the pointer has rested long enough for the tooltip to show at `now`.
    pub(crate) fn is_due(&self, now: Instant) -> bool {
        self.since + DELAY <= now
    }
}

/// Paints `text` over a layer of its own, below `bounds` or above it when there is no room.
pub(crate) fn paint_tooltip<M>(cx: &mut PaintContext<'_, '_, M>, bounds: Rect, text: &str) {
    let theme = *cx.theme();
    let font = theme.text.sm;
    let run = cx.shape(text, font);
    let width = run.width + PADDING * 2.0;
    let height = run.height + PADDING;
    let viewport = cx.viewport();
    let left = bounds
        .left()
        .min(viewport.right() - width - GAP)
        .max(viewport.left());
    let below = bounds.bottom() + GAP;
    let top = if below + height <= viewport.bottom() {
        below
    } else {
        (bounds.top() - height - GAP).max(viewport.top())
    };
    cx.push_layer();
    cx.quad(
        Quad::filled(
            Rect::from_xywh(left, top, width, height),
            theme.colors.surface,
        )
        .corner_radius(theme.radius.sm)
        .border(1.0, theme.colors.border),
    );
    cx.text(
        Point::new(left + PADDING, top + PADDING * 0.5),
        run,
        theme.colors.text,
    );
    cx.pop_layer();
}
