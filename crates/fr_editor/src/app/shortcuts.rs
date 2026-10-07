//! The keyboard shortcuts of the editor.

use fr_input::{ButtonState, Key, KeyEvent};

use super::message::Action;

/// The action a key press asks for, when it is a shortcut.
pub fn action_for(event: &KeyEvent) -> Option<Action> {
    if event.state != ButtonState::Pressed {
        return None;
    }
    let modifiers = event.modifiers;
    let command = modifiers.control || modifiers.logo;
    match &event.key {
        Key::Character(text) if command => command_key(&text.to_lowercase(), modifiers.shift),
        Key::Function(5) if modifiers.shift => Some(Action::Stop),
        Key::Function(5) => Some(Action::Play { build_first: false }),
        Key::Function(2) => Some(Action::Rename),
        Key::Delete if !event.repeat => Some(Action::Delete),
        _ => None,
    }
}

/// The action of a letter pressed with Control or Command.
fn command_key(letter: &str, shift: bool) -> Option<Action> {
    match (letter, shift) {
        ("s", false) => Some(Action::Save),
        ("s", true) => Some(Action::SaveAll),
        ("z", false) => Some(Action::Undo),
        ("z", true) | ("y", _) => Some(Action::Redo),
        ("d", _) => Some(Action::Duplicate),
        ("o", _) => Some(Action::OpenProject),
        ("n", false) => Some(Action::NewScene),
        ("n", true) => Some(Action::NewPrefab),
        ("q", _) => Some(Action::Quit),
        _ => None,
    }
}
