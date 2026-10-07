//! Conversions from winit's input types to the value types of `fr_input`.

use fr_input::{ButtonState, Key, Modifiers, PointerButton, ScrollDelta};
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};

/// How many logical pixels one line of a wheel scroll stands for.
const LINE_PIXELS: f32 = 40.0;

/// The button winit reports as `button`.
pub(crate) fn pointer_button(button: MouseButton) -> PointerButton {
    match button {
        MouseButton::Left => PointerButton::Primary,
        MouseButton::Right => PointerButton::Secondary,
        MouseButton::Middle => PointerButton::Middle,
        MouseButton::Back => PointerButton::Other(8),
        MouseButton::Forward => PointerButton::Other(9),
        MouseButton::Other(number) => PointerButton::Other(number),
    }
}

/// The state winit reports as `state`.
pub(crate) fn button_state(state: ElementState) -> ButtonState {
    match state {
        ElementState::Pressed => ButtonState::Pressed,
        ElementState::Released => ButtonState::Released,
    }
}

/// The delta winit reports as `delta` on a window at `scale_factor`.
pub(crate) fn scroll_delta(delta: MouseScrollDelta, scale_factor: f64) -> ScrollDelta {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => ScrollDelta {
            x: x * LINE_PIXELS,
            y: y * LINE_PIXELS,
        },
        MouseScrollDelta::PixelDelta(position) => ScrollDelta {
            x: (position.x / scale_factor) as f32,
            y: (position.y / scale_factor) as f32,
        },
    }
}

/// The modifiers winit reports as `state`.
pub(crate) fn modifiers(state: ModifiersState) -> Modifiers {
    Modifiers {
        shift: state.shift_key(),
        control: state.control_key(),
        alt: state.alt_key(),
        logo: state.super_key(),
    }
}

/// The number of the function key `named` is, when it is one of F1 to F12.
fn function_key(named: &NamedKey) -> Option<u8> {
    match named {
        NamedKey::F1 => Some(1),
        NamedKey::F2 => Some(2),
        NamedKey::F3 => Some(3),
        NamedKey::F4 => Some(4),
        NamedKey::F5 => Some(5),
        NamedKey::F6 => Some(6),
        NamedKey::F7 => Some(7),
        NamedKey::F8 => Some(8),
        NamedKey::F9 => Some(9),
        NamedKey::F10 => Some(10),
        NamedKey::F11 => Some(11),
        NamedKey::F12 => Some(12),
        _ => None,
    }
}

/// The key winit reports as `key`.
pub(crate) fn key(key: &WinitKey) -> Key {
    match key {
        WinitKey::Character(text) => Key::Character(text.to_string()),
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Tab) => Key::Tab,
        WinitKey::Named(NamedKey::Space) => Key::Space,
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::Delete) => Key::Delete,
        WinitKey::Named(NamedKey::ArrowUp) => Key::ArrowUp,
        WinitKey::Named(NamedKey::ArrowDown) => Key::ArrowDown,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::ArrowLeft,
        WinitKey::Named(NamedKey::ArrowRight) => Key::ArrowRight,
        WinitKey::Named(NamedKey::Home) => Key::Home,
        WinitKey::Named(NamedKey::End) => Key::End,
        WinitKey::Named(named) => function_key(named).map_or(Key::Other, Key::Function),
        _ => Key::Other,
    }
}
