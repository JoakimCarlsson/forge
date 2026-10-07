//! The window's UI state: input, focus, capture and the regions the last frame left.

use std::time::{Duration, Instant};

use fr_input::{ButtonState, Key, KeyEvent, Modifiers, PointerButton, ScrollDelta};
use fr_math::{Point, Rect, Size};
use fr_render::{DrawList, TextSystem};

use crate::canvas::Canvas;
use crate::drag::{DragEvent, DragPhase};
use crate::element::{Element, Input, LayoutContext, PaintContext};
use crate::region::{DragHandler, Region, RegionAction};
use crate::theme::Theme;
use crate::tooltip::{HeldTooltip, paint_tooltip};

/// How far the pointer travels from a press before the press becomes a drag.
const DRAG_THRESHOLD: f32 = 3.0;

/// The longest gap between two clicks that make a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// How far a second click may land from the first and still be a double click.
const DOUBLE_CLICK_SLOP: f32 = 4.0;

/// A press of the primary button that has not been released yet.
struct Press<M> {
    /// Where the pointer went down.
    start: Point,
    /// Where the pointer was at the previous event of the press.
    last: Point,
    /// Where the region that took the press was painted.
    bounds: Rect,
    /// What a click on that region sends.
    click: Option<M>,
    /// What a second click on it sends.
    double_click: Option<M>,
    /// What the drag it captured sends, when it captured one.
    drag: Option<DragHandler<M>>,
    /// Whether the pointer has travelled far enough for this to be a drag.
    began: bool,
}

impl<M> Press<M> {
    /// One event of the drag this press captured, ending at `position`.
    ///
    /// The press remembers `position`, so the next event's delta is measured
    /// from it.
    fn event(&mut self, phase: DragPhase, position: Point, modifiers: Modifiers) -> DragEvent {
        let delta = Point::new(position.x - self.last.x, position.y - self.last.y);
        self.last = position;
        DragEvent {
            phase,
            start: self.start,
            position,
            delta,
            modifiers,
        }
    }
}

/// The last completed click, which the next may turn into a double click.
struct LastClick {
    /// Where the region clicked was painted.
    bounds: Rect,
    /// Where the pointer was released.
    at: Point,
    /// When.
    when: Instant,
}

/// Everything that survives between frames: the theme, the pointer and focus.
///
/// The element tree does not survive: the caller rebuilds it every frame and
/// hands it to [`Ui::draw`]. What the tree leaves behind is a list of regions,
/// in paint order, which is how a click or a keypress after the frame turns
/// into one of the caller's messages. A press that lands on a region captures
/// the pointer: the drag it starts is delivered to that region until the button
/// comes up, wherever the pointer goes.
pub struct Ui<M> {
    /// The tokens frames are drawn from.
    theme: Theme,
    /// What the pointer is doing.
    input: Input,
    /// The modifier keys as of the last key event.
    modifiers: Modifiers,
    /// The region holding keyboard focus, as an index into `regions`.
    focus: Option<usize>,
    /// The regions painted by the last frame, in paint order.
    regions: Vec<Region<M>>,
    /// The layered recording a frame is painted on before it reaches the draw list.
    canvas: Canvas,
    /// The press of the primary button in progress.
    press: Option<Press<M>>,
    /// The click that completed last.
    last_click: Option<LastClick>,
    /// The tooltip under the pointer, shown once it has been held long enough.
    tooltip: Option<HeldTooltip>,
}

impl<M> Ui<M> {
    /// Creates UI state drawing in `theme`, with nothing focused.
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            input: Input::default(),
            modifiers: Modifiers::default(),
            focus: None,
            regions: Vec::new(),
            canvas: Canvas::new(Rect::default()),
            press: None,
            last_click: None,
            tooltip: None,
        }
    }

    /// The tokens frames are drawn from.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Draws later frames in `theme`.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// The modifier keys as the last key event reported them.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Records the pointer at `pointer`, in logical pixels.
    ///
    /// While a press that captured a drag is held this is also how the drag
    /// begins and moves, so the message it returns is the drag's.
    pub fn pointer_moved(&mut self, pointer: Point) -> Option<M> {
        self.input.pointer = Some(pointer);
        self.tooltip = None;
        let modifiers = self.modifiers;
        let press = self.press.as_mut()?;
        let handler = press.drag.clone()?;
        if !press.began {
            let moved = (pointer.x - press.start.x).abs() + (pointer.y - press.start.y).abs();
            if moved < DRAG_THRESHOLD {
                return None;
            }
            press.began = true;
            self.input.dragging = true;
            return Some(handler(press.event(DragPhase::Began, pointer, modifiers)));
        }
        Some(handler(press.event(DragPhase::Moved, pointer, modifiers)))
    }

    /// Records the pointer leaving the window, keeping a captured drag alive.
    pub fn pointer_left(&mut self) {
        self.input.pointer = None;
        self.tooltip = None;
    }

    /// Routes a pointer button going down or coming up.
    ///
    /// The primary button going down returns the message of a press handler;
    /// coming up it returns the click, double click or the end of the drag it
    /// completed. The secondary button going down returns the secondary message
    /// of the region under the pointer, which is what opens a context menu.
    pub fn pointer_button(&mut self, button: PointerButton, state: ButtonState) -> Option<M>
    where
        M: Clone,
    {
        match (button, state) {
            (PointerButton::Primary, ButtonState::Pressed) => self.pointer_pressed(),
            (PointerButton::Primary, ButtonState::Released) => self.pointer_released(),
            (PointerButton::Secondary, ButtonState::Pressed) => self.secondary_pressed(),
            _ => None,
        }
    }

    /// Routes a turn of the wheel to the scroll area under the pointer.
    ///
    /// Returns the message of the topmost region under the pointer, on the top
    /// layer there, that listens to the wheel.
    pub fn scrolled(&self, delta: ScrollDelta) -> Option<M> {
        let pointer = self.input.pointer?;
        let handler = self.handler_at(pointer, |action| action.scroll.clone())?;
        Some(handler(delta))
    }

    /// Routes a key: tab and shift-tab move focus, escape abandons a drag or drops
    /// focus, and enter or space returns the message of the focused region.
    pub fn key(&mut self, event: &KeyEvent) -> Option<M>
    where
        M: Clone,
    {
        self.modifiers = event.modifiers;
        if event.state != ButtonState::Pressed {
            return None;
        }
        match event.key {
            Key::Tab if event.modifiers.shift => self.focus_previous(),
            Key::Tab => self.focus_next(),
            Key::Escape if self.press.is_some() => return self.pointer_cancelled(),
            Key::Escape => self.clear_focus(),
            Key::Enter | Key::Space if !event.repeat => return self.activate_focused(),
            _ => {}
        }
        None
    }

    /// Abandons the press in progress, as a window losing the pointer does.
    ///
    /// A drag that had begun hears `Cancelled`; a press that had not is dropped.
    pub fn pointer_cancelled(&mut self) -> Option<M> {
        self.input.pressed_at = None;
        self.input.drag_bounds = None;
        self.input.dragging = false;
        let mut press = self.press.take()?;
        if !press.began {
            return None;
        }
        let handler = press.drag.clone()?;
        let position = self.input.pointer.unwrap_or(press.last);
        Some(handler(press.event(
            DragPhase::Cancelled,
            position,
            self.modifiers,
        )))
    }

    /// Records a press, giving it to the topmost region under the pointer.
    fn pointer_pressed(&mut self) -> Option<M>
    where
        M: Clone,
    {
        self.focus = None;
        self.tooltip = None;
        let pointer = self.input.pointer?;
        self.input.pressed_at = Some(pointer);
        self.input.drag_bounds = None;
        let region = &self.regions[self.region_at(pointer)?];
        let action = &region.action;
        let message = action.press.as_ref().map(|press| press(pointer));
        self.input.drag_bounds = action.drag.is_some().then_some(region.bounds);
        self.press = Some(Press {
            start: pointer,
            last: pointer,
            bounds: region.bounds,
            click: action.click.clone(),
            double_click: action.double_click.clone(),
            drag: action.drag.clone(),
            began: false,
        });
        message
    }

    /// Records a release, returning the message of what it completed.
    ///
    /// A click completes when the press and the release both land in the
    /// topmost region under them.
    fn pointer_released(&mut self) -> Option<M>
    where
        M: Clone,
    {
        self.input.pressed_at = None;
        self.input.drag_bounds = None;
        self.input.dragging = false;
        let mut press = self.press.take()?;
        if press.began {
            let handler = press.drag.clone()?;
            let position = self.input.pointer.unwrap_or(press.last);
            return Some(handler(press.event(
                DragPhase::Ended,
                position,
                self.modifiers,
            )));
        }
        let pointer = self.input.pointer?;
        let region = &self.regions[self.region_at(pointer)?];
        if !region.bounds.contains(press.start) {
            return None;
        }
        self.completed_click(press, pointer)
    }

    /// The message of a click that completed at `pointer`: the double click
    /// when it follows the last one closely enough and the region has one.
    fn completed_click(&mut self, press: Press<M>, pointer: Point) -> Option<M> {
        let now = Instant::now();
        let doubled = self.last_click.as_ref().is_some_and(|last| {
            last.bounds == press.bounds
                && now.duration_since(last.when) <= DOUBLE_CLICK
                && (last.at.x - pointer.x).abs() + (last.at.y - pointer.y).abs()
                    <= DOUBLE_CLICK_SLOP
        });
        if doubled && press.double_click.is_some() {
            self.last_click = None;
            return press.double_click;
        }
        self.last_click = Some(LastClick {
            bounds: press.bounds,
            at: pointer,
            when: now,
        });
        press.click
    }

    /// The secondary message of the region under a press of the secondary button.
    ///
    /// A menu opens under the pointer the moment the button goes down, the way
    /// every other editor opens one, so there is no release to wait for.
    fn secondary_pressed(&mut self) -> Option<M>
    where
        M: Clone,
    {
        self.focus = None;
        let pointer = self.input.pointer?;
        self.handler_at(pointer, |action| action.secondary.clone())
    }

    /// Whether the pointer is over a region from the last frame.
    pub fn pointer_over_region(&self) -> bool {
        self.input
            .pointer
            .is_some_and(|pointer| self.region_at(pointer).is_some())
    }

    /// Whether a press that began on a region holds the pointer.
    pub fn pointer_captured(&self) -> bool {
        self.press.is_some()
    }

    /// Whether the held pointer is captured by a region that takes drags, from
    /// the press until the release, so the host can keep it from the scene.
    pub fn dragging(&self) -> bool {
        self.press
            .as_ref()
            .is_some_and(|press| press.drag.is_some())
    }

    /// Where the pointer is, if it is over the window.
    pub fn pointer(&self) -> Option<Point> {
        self.input.pointer
    }

    /// Moves focus to the next click target in tab order, wrapping around.
    pub fn focus_next(&mut self) {
        self.focus = self.step_focus(1);
    }

    /// Moves focus to the previous click target in tab order, wrapping around.
    pub fn focus_previous(&mut self) {
        self.focus = self.step_focus(-1);
    }

    /// Gives up focus entirely.
    pub fn clear_focus(&mut self) {
        self.focus = None;
    }

    /// The message of the focused region, for a key that activates it.
    pub fn activate_focused(&self) -> Option<M>
    where
        M: Clone,
    {
        self.regions.get(self.focus?)?.action.click.clone()
    }

    /// When the tooltip under the pointer is due, until a frame shows it.
    pub fn next_tooltip(&self) -> Option<Instant> {
        self.tooltip
            .as_ref()
            .map(|held| held.since + crate::tooltip::DELAY)
    }

    /// Measures `root` against `offer`, paints it at `origin` and reports its size.
    ///
    /// The frame is recorded on a layered canvas and replayed into `list` at
    /// the end, so a popup painted on a layer of its own draws over the text of
    /// the screen under it.
    pub fn draw(
        &mut self,
        text: &mut TextSystem,
        list: &mut DrawList,
        offer: Size,
        origin: Point,
        mut root: impl Element<M>,
    ) -> Size {
        let pointer_layer = self
            .input
            .pointer
            .and_then(|pointer| self.region_at(pointer))
            .map_or(0, |index| self.regions[index].layer);
        self.regions.clear();
        self.canvas.reset(list.viewport());

        let mut layout = LayoutContext::new(&self.theme, text);
        let size = root.measure(offer, &mut layout);

        let mut cx = PaintContext::new(
            layout,
            &mut self.canvas,
            self.input,
            self.focus,
            pointer_layer,
            &mut self.regions,
        );
        root.paint(Rect::new(origin, size), &mut cx);

        let now = Instant::now();
        self.tooltip = HeldTooltip::hold(self.tooltip.take(), cx.take_tooltip(), now);
        if let Some(held) = self.tooltip.as_ref().filter(|held| held.is_due(now)) {
            paint_tooltip(&mut cx, held.bounds, &held.text);
        }
        self.canvas.flush(list);

        if self.focus.is_some_and(|index| index >= self.regions.len()) {
            self.focus = None;
        }
        size
    }

    /// The topmost region containing `point`: the highest layer, then the last painted.
    fn region_at(&self, point: Point) -> Option<usize> {
        self.regions
            .iter()
            .enumerate()
            .filter(|(_, region)| region.bounds.contains(point))
            .max_by_key(|(index, region)| (region.layer, *index))
            .map(|(index, _)| index)
    }

    /// The first thing `pick` takes from the regions under `point` on its top layer,
    /// topmost first.
    fn handler_at<T>(
        &self,
        point: Point,
        pick: impl Fn(&RegionAction<M>) -> Option<T>,
    ) -> Option<T> {
        let layer = self.regions[self.region_at(point)?].layer;
        self.regions
            .iter()
            .rev()
            .filter(|region| region.layer == layer && region.bounds.contains(point))
            .find_map(|region| pick(&region.action))
    }

    /// The highest layer any region of the last frame was painted on.
    fn top_layer(&self) -> u32 {
        self.regions
            .iter()
            .map(|region| region.layer)
            .max()
            .unwrap_or(0)
    }

    /// Focus moved by `step` places in tab order among the top layer's click
    /// targets, wrapping around.
    fn step_focus(&self, step: isize) -> Option<usize> {
        let count = self.regions.len() as isize;
        if count == 0 {
            return None;
        }
        let top = self.top_layer();
        let base = match self.focus {
            Some(index) => index as isize,
            None if step >= 0 => -1,
            None => count,
        };
        (1..=count).find_map(|distance| {
            let next = (base + step * distance).rem_euclid(count) as usize;
            let region = &self.regions[next];
            (region.layer == top
                && region.action.is_focusable()
                && region.bounds.size.width > 0.0
                && region.bounds.size.height > 0.0)
                .then_some(next)
        })
    }
}
