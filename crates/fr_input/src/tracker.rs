//! What the pointer and keyboard did over a frame, gathered from events.

use crate::keyboard::KeyEvent;
use crate::pointer::{PointerButton, ScrollDelta};
use crate::state::ButtonState;

/// The state of the pointer and the keyboard that a game reads each frame.
///
/// Events are fed in as they arrive; [`InputState::end_frame`] clears what only
/// lasts a frame, the pointer's movement, the scroll and the key events. The
/// position and the held buttons carry over.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputState {
    /// The pointer position in logical pixels, when it is inside the window.
    pointer: Option<[f32; 2]>,
    /// How far the pointer moved this frame, in logical pixels.
    pointer_delta: [f32; 2],
    /// The buttons held down.
    buttons: Vec<PointerButton>,
    /// How far the wheel scrolled this frame.
    scroll: ScrollDelta,
    /// The key events of this frame in order.
    keys: Vec<KeyEvent>,
}

impl InputState {
    /// An input state with nothing held and the pointer outside the window.
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves the pointer to a position inside the window.
    pub fn pointer_moved(&mut self, x: f32, y: f32) {
        if let Some([from_x, from_y]) = self.pointer {
            self.pointer_delta[0] += x - from_x;
            self.pointer_delta[1] += y - from_y;
        }
        self.pointer = Some([x, y]);
    }

    /// Takes the pointer out of the window.
    pub fn pointer_left(&mut self) {
        self.pointer = None;
    }

    /// Records a button going down or coming up.
    pub fn button(&mut self, button: PointerButton, state: ButtonState) {
        self.buttons.retain(|held| *held != button);
        if state == ButtonState::Pressed {
            self.buttons.push(button);
        }
    }

    /// Adds a scroll.
    pub fn scrolled(&mut self, delta: ScrollDelta) {
        self.scroll.x += delta.x;
        self.scroll.y += delta.y;
    }

    /// Records a key event.
    pub fn key(&mut self, event: KeyEvent) {
        self.keys.push(event);
    }

    /// Clears what lasts one frame: movement, scroll and key events.
    pub fn end_frame(&mut self) {
        self.pointer_delta = [0.0, 0.0];
        self.scroll = ScrollDelta::default();
        self.keys.clear();
    }

    /// The pointer position in logical pixels, when it is inside the window.
    pub fn pointer(&self) -> Option<[f32; 2]> {
        self.pointer
    }

    /// How far the pointer moved this frame, in logical pixels.
    pub fn pointer_delta(&self) -> [f32; 2] {
        self.pointer_delta
    }

    /// Whether a button is held down.
    pub fn button_down(&self, button: PointerButton) -> bool {
        self.buttons.contains(&button)
    }

    /// How far the wheel scrolled this frame.
    pub fn scroll(&self) -> ScrollDelta {
        self.scroll
    }

    /// The key events of this frame in order.
    pub fn key_events(&self) -> &[KeyEvent] {
        &self.keys
    }
}
