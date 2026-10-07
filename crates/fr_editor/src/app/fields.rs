//! Text fields of the interface: a field shows the text its owner keeps, and
//! turns into the live edit while it has the keyboard.

use fr_ui::{Field, field, text_field};

use super::message::{Message, TextMessage, TextTarget};
use super::state::EditorState;

/// A field for `target` showing `value`, or the edit in progress when the
/// field has the keyboard. Presses and drags come back as [`Message::Text`].
pub fn text_input(state: &EditorState, target: &TextTarget, value: &str) -> Field<Message> {
    let focused = state.text.has(target);
    let base: Field<Message> = if focused {
        text_field(&state.text.edit, true)
    } else {
        field(value, value.chars().count(), false)
    };
    let pressed = target.clone();
    let dragged = target.clone();
    base.on_press(move |caret| {
        Message::Text(TextMessage::Press {
            target: pressed.clone(),
            caret,
        })
    })
    .on_drag(move |caret| {
        Message::Text(TextMessage::Drag {
            target: dragged.clone(),
            caret,
        })
    })
}
