//! What the window's events leave for the viewport between two frames.

use fr_input::{ButtonState, KeyEvent, Modifiers, PointerButton};
use fr_math::{Point, Rect};

use crate::viewport::{ButtonSet, ViewportInput, ViewportKey};

/// The pointer and keys the viewport has been given since its last update.
#[derive(Debug, Default)]
pub struct ViewportFeed {
    /// Where the pointer is, when it is over the window.
    pointer: Option<Point>,
    /// The buttons held that went down over the viewport.
    held: ButtonSet,
    /// The buttons that went down over the viewport this frame.
    pressed: ButtonSet,
    /// The buttons that came up this frame.
    released: ButtonSet,
    /// The wheel since the last update, in notches.
    scroll: f32,
    /// The viewport keys that went down this frame.
    keys_pressed: Vec<ViewportKey>,
    /// The viewport keys held now.
    keys_held: Vec<ViewportKey>,
}

impl ViewportFeed {
    /// Records the pointer.
    pub fn moved(&mut self, pointer: Point) {
        self.pointer = Some(pointer);
    }

    /// Records the pointer leaving the window.
    pub fn left(&mut self) {
        self.pointer = None;
    }

    /// Records a button going down over the viewport.
    pub fn press(&mut self, button: PointerButton) {
        self.held.set(button, true);
        self.pressed.set(button, true);
    }

    /// Records a button coming up; a release always reaches the viewport so a
    /// drag is never left half done.
    pub fn release(&mut self, button: PointerButton) {
        if self.held.any() {
            self.held.set(button, false);
            self.released.set(button, true);
        }
    }

    /// Adds turns of the wheel.
    pub fn scrolled(&mut self, notches: f32) {
        self.scroll += notches;
    }

    /// Records a key press or release; keys that are not the viewport's are
    /// ignored.
    pub fn key(&mut self, event: &KeyEvent) {
        let Some(key) = ViewportKey::from_key(&event.key) else {
            return;
        };
        match event.state {
            ButtonState::Pressed => {
                if !event.repeat {
                    self.keys_pressed.push(key);
                }
                if !self.keys_held.contains(&key) {
                    self.keys_held.push(key);
                }
            }
            ButtonState::Released => self.keys_held.retain(|held| *held != key),
        }
    }

    /// Forgets the keys held, as when a text field takes the keyboard.
    pub fn release_keys(&mut self) {
        self.keys_held.clear();
        self.keys_pressed.clear();
    }

    /// Whether a button that went down over the viewport is still held.
    pub fn holding(&self) -> bool {
        self.held.any()
    }

    /// The input of one frame, leaving what happens once for the next.
    pub fn take(
        &mut self,
        rect: Rect,
        hovered: bool,
        modifiers: Modifiers,
        delta_time: f32,
    ) -> ViewportInput {
        let input = ViewportInput {
            rect,
            pointer: self.pointer,
            hovered,
            held: self.held,
            pressed: self.pressed,
            released: self.released,
            scroll: if hovered { self.scroll } else { 0.0 },
            modifiers,
            keys_pressed: std::mem::take(&mut self.keys_pressed),
            keys_held: self.keys_held.clone(),
            delta_time,
        };
        self.pressed = ButtonSet::default();
        self.released = ButtonSet::default();
        self.scroll = 0.0;
        input
    }
}
