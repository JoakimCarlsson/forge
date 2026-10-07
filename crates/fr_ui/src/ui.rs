//! The window's UI state: input, focus and the regions the last frame left.

use fr_input::{ButtonState, Key, KeyEvent, PointerButton};
use fr_math::{Point, Rect, Size};
use fr_render::{DrawList, TextSystem};

use crate::element::{Element, Input, LayoutContext, PaintContext, Region, RegionAction};
use crate::theme::Theme;

/// Everything that survives between frames: the theme, the pointer and focus.
///
/// The element tree does not survive: the caller rebuilds it every frame and
/// hands it to [`Ui::draw`]. What the tree leaves behind is a list of regions,
/// in paint order, which is how a click or a keypress after the frame turns
/// into one of the caller's messages.
pub struct Ui<M> {
    /// The tokens frames are drawn from.
    theme: Theme,
    /// What the pointer is doing.
    input: Input,
    /// The region holding keyboard focus, as an index into `regions`.
    focus: Option<usize>,
    /// The regions painted by the last frame, in paint order.
    regions: Vec<Region<M>>,
}

impl<M> Ui<M> {
    /// Creates UI state drawing in `theme`, with nothing focused.
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            input: Input::default(),
            focus: None,
            regions: Vec::new(),
        }
    }

    /// The tokens frames are drawn from.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Records the pointer at `pointer`, in logical pixels.
    pub fn pointer_moved(&mut self, pointer: Point) {
        self.input.pointer = Some(pointer);
    }

    /// Records the pointer leaving the window.
    pub fn pointer_left(&mut self) {
        self.input.pointer = None;
        self.input.pressed_at = None;
    }

    /// Routes a pointer button going down or coming up, returning the message of the region a
    /// primary click completed on.
    pub fn pointer_button(&mut self, button: PointerButton, state: ButtonState) -> Option<M>
    where
        M: Clone,
    {
        if button != PointerButton::Primary {
            return None;
        }
        match state {
            ButtonState::Pressed => {
                self.pointer_pressed();
                None
            }
            ButtonState::Released => self.pointer_released(),
        }
    }

    /// Routes a key: tab and shift-tab move focus, escape drops it, and enter or space
    /// returns the message of the focused region.
    pub fn key(&mut self, event: &KeyEvent) -> Option<M>
    where
        M: Clone,
    {
        if event.state != ButtonState::Pressed {
            return None;
        }
        match event.key {
            Key::Tab if event.modifiers.shift => self.focus_previous(),
            Key::Tab => self.focus_next(),
            Key::Escape => self.clear_focus(),
            Key::Enter | Key::Space if !event.repeat => return self.activate_focused(),
            _ => {}
        }
        None
    }

    /// Records a press and leaves keyboard focus to keyboard navigation.
    fn pointer_pressed(&mut self) {
        self.input.pressed_at = self.input.pointer;
        self.focus = None;
    }

    /// Records a release, returning the message of the region it completed on.
    ///
    /// A click completes when the press and the release both land in the
    /// topmost region under them.
    fn pointer_released(&mut self) -> Option<M>
    where
        M: Clone,
    {
        let pressed_at = self.input.pressed_at.take()?;
        let pointer = self.input.pointer?;
        let region = &self.regions[self.region_at(pointer)?];
        match &region.action {
            RegionAction::Click(message) if region.bounds.contains(pressed_at) => {
                Some(message.clone())
            }
            RegionAction::Click(_) | RegionAction::Inert => None,
        }
    }

    /// Moves focus to the next click target in tab order, wrapping around.
    fn focus_next(&mut self) {
        self.focus = self.step_focus(1);
    }

    /// Moves focus to the previous click target in tab order, wrapping around.
    fn focus_previous(&mut self) {
        self.focus = self.step_focus(-1);
    }

    /// Gives up focus entirely.
    fn clear_focus(&mut self) {
        self.focus = None;
    }

    /// The message of the focused region, for a key that activates it.
    fn activate_focused(&self) -> Option<M>
    where
        M: Clone,
    {
        let index = self.focus?;
        match &self.regions.get(index)?.action {
            RegionAction::Click(message) => Some(message.clone()),
            RegionAction::Inert => None,
        }
    }

    /// Whether the pointer is over a region from the last frame.
    pub fn pointer_over_region(&self) -> bool {
        self.input
            .pointer
            .is_some_and(|pointer| self.region_at(pointer).is_some())
    }

    /// Measures `root` against `offer`, paints it at `origin` and reports its size.
    pub fn draw(
        &mut self,
        text: &mut TextSystem,
        list: &mut DrawList,
        offer: Size,
        origin: Point,
        mut root: impl Element<M>,
    ) -> Size {
        self.regions.clear();

        let mut layout = LayoutContext::new(&self.theme, text);
        let size = root.measure(offer, &mut layout);

        let mut cx = PaintContext::new(layout, list, self.input, self.focus, &mut self.regions);
        root.paint(Rect::new(origin, size), &mut cx);

        if self.focus.is_some_and(|index| index >= self.regions.len()) {
            self.focus = None;
        }
        size
    }

    /// The topmost region containing `point`.
    fn region_at(&self, point: Point) -> Option<usize> {
        self.regions
            .iter()
            .rposition(|region| region.bounds.contains(point))
    }

    /// Focus moved by `step` places in tab order, wrapping around.
    fn step_focus(&self, step: isize) -> Option<usize> {
        let count = self.regions.len() as isize;
        if count == 0 {
            return None;
        }
        let base = match self.focus {
            Some(index) => index as isize,
            None if step >= 0 => -1,
            None => count,
        };
        (1..=count).find_map(|distance| {
            let next = (base + step * distance).rem_euclid(count) as usize;
            matches!(self.regions[next].action, RegionAction::Click(_)).then_some(next)
        })
    }
}
