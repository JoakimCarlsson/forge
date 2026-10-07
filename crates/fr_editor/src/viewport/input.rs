//! What the app shell tells the viewport each frame, and what it answers.
//!
//! The viewport never reads the UI: the caller decides whether the pointer is
//! over the viewport and free of overlays, and which keys apply, and fills a
//! [`ViewportInput`] accordingly.

use fr_input::{Key, Modifiers, PointerButton};
use fr_math::{Point, Rect};

use super::transform_gizmo::Tool;

/// A key the viewport reacts to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewportKey {
    /// Select tool, and moving down while flying.
    Q,
    /// Move tool, and moving forward while flying.
    W,
    /// Rotate tool, and moving up while flying.
    E,
    /// Scale tool.
    R,
    /// Moving left while flying.
    A,
    /// Moving backward while flying.
    S,
    /// Moving right while flying.
    D,
    /// Frame the selection.
    F,
    /// Switch between local and world handles.
    X,
    /// Cancel the drag in progress.
    Escape,
}

impl ViewportKey {
    /// The viewport key a keyboard key stands for, when it is one.
    pub fn from_key(key: &Key) -> Option<Self> {
        match key {
            Key::Escape => Some(Self::Escape),
            Key::Character(text) => match text.to_ascii_lowercase().as_str() {
                "q" => Some(Self::Q),
                "w" => Some(Self::W),
                "e" => Some(Self::E),
                "r" => Some(Self::R),
                "a" => Some(Self::A),
                "s" => Some(Self::S),
                "d" => Some(Self::D),
                "f" => Some(Self::F),
                "x" => Some(Self::X),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Which pointer buttons a set holds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ButtonSet {
    /// The main button.
    pub primary: bool,
    /// The context button.
    pub secondary: bool,
    /// The wheel button.
    pub middle: bool,
}

impl ButtonSet {
    /// Adds or removes a button; buttons the viewport has no use for are
    /// ignored.
    pub fn set(&mut self, button: PointerButton, held: bool) {
        match button {
            PointerButton::Primary => self.primary = held,
            PointerButton::Secondary => self.secondary = held,
            PointerButton::Middle => self.middle = held,
            PointerButton::Other(_) => {}
        }
    }

    /// Whether any button is in the set.
    pub fn any(&self) -> bool {
        self.primary || self.secondary || self.middle
    }
}

/// The state of the pointer and keyboard for one frame of the viewport.
#[derive(Clone, Debug, Default)]
pub struct ViewportInput {
    /// The rectangle of the window the viewport occupies, in logical pixels.
    pub rect: Rect,
    /// The pointer in window logical pixels, when it is known.
    pub pointer: Option<Point>,
    /// Whether the pointer is over the viewport rectangle and free of any UI
    /// overlay. A drag already in progress continues without it.
    pub hovered: bool,
    /// The buttons held now.
    pub held: ButtonSet,
    /// The buttons that went down this frame.
    pub pressed: ButtonSet,
    /// The buttons that came up this frame.
    pub released: ButtonSet,
    /// The wheel in notches, positive away from the user, which zooms in.
    pub scroll: f32,
    /// The modifier keys held.
    pub modifiers: Modifiers,
    /// The viewport keys that went down this frame; the caller leaves it empty
    /// while a text field has the keyboard.
    pub keys_pressed: Vec<ViewportKey>,
    /// The viewport keys held now, which move the camera while flying.
    pub keys_held: Vec<ViewportKey>,
    /// The seconds since the previous frame.
    pub delta_time: f32,
}

/// The cursor the viewport would like shown.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorHint {
    /// The ordinary pointer.
    #[default]
    Default,
    /// Over a handle of the transform gizmo, or dragging one.
    Handle,
    /// Orbiting the camera.
    Orbit,
    /// Panning the camera.
    Pan,
    /// Looking around while flying.
    Look,
}

/// What the viewport did with a frame of input.
#[derive(Clone, Debug)]
pub struct ViewportOutput {
    /// Whether the viewport is using the pointer, by a drag or by hovering a
    /// handle, so the rest of the interface should not react to it.
    pub captured: bool,
    /// The cursor to show.
    pub cursor: CursorHint,
    /// Whether the selection changed.
    pub selection_changed: bool,
    /// Whether a gizmo drag changed the document this frame.
    pub edited: bool,
    /// Whether a gizmo drag was committed as an undo entry this frame.
    pub committed: bool,
    /// The tool in force after the frame.
    pub tool: Tool,
    /// Failures to show in the console.
    pub messages: Vec<String>,
}

impl ViewportOutput {
    /// An output that did nothing, with the tool in force.
    pub fn idle(tool: Tool) -> Self {
        Self {
            captured: false,
            cursor: CursorHint::Default,
            selection_changed: false,
            edited: false,
            committed: false,
            tool,
            messages: Vec::new(),
        }
    }
}
