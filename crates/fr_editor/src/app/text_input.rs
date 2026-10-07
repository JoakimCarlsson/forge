//! Typing: which text field has the keyboard, what a keystroke does to it and
//! what leaving it means.

use fr_input::KeyEvent;
use fr_ui::ClipboardRequest;

use super::message::{PickerMessage, TextTarget};
use super::picker;
use super::state::EditorState;
use crate::panels::hierarchy_update::{cancel_rename, commit_rename};
use crate::panels::text_end::TextEnd;
use crate::panels::{inspector, project};

/// The text a field starts with when a press gives it the keyboard.
fn source_text(state: &EditorState, target: &TextTarget) -> String {
    match target {
        TextTarget::HierarchyFilter => state.hierarchy.filter.clone(),
        TextTarget::PickerRoot | TextTarget::PickerOpenPath | TextTarget::PickerName => state
            .picker
            .as_ref()
            .and_then(|picker| picker.field_text(target))
            .unwrap_or_default()
            .to_owned(),
        TextTarget::Project(_) => project::text_source(state, target),
        TextTarget::Inspector(_) => inspector::text_source(state, target),
        TextTarget::HierarchyRename(_) => String::new(),
    }
}

/// Copies the edit into the place the field's owner keeps its text, so a field
/// that changes as it is typed in follows every keystroke.
fn after_change(state: &mut EditorState) {
    let Some(target) = state.text.target.clone() else {
        return;
    };
    let text = state.text.edit.text().to_owned();
    match &target {
        TextTarget::HierarchyFilter => state.hierarchy.filter = text,
        TextTarget::PickerRoot | TextTarget::PickerOpenPath | TextTarget::PickerName => {
            if let Some(picker) = &mut state.picker {
                picker.set_field_text(&target, &text);
            }
        }
        TextTarget::HierarchyRename(_) | TextTarget::Project(_) | TextTarget::Inspector(_) => {}
    }
}

/// Takes the keyboard from the field; `end` says why: Enter and a click
/// elsewhere apply what was typed, Escape throws it away.
pub fn finish(state: &mut EditorState, end: TextEnd) {
    let Some((target, text)) = state.text.end() else {
        return;
    };
    match (&target, end) {
        (TextTarget::HierarchyRename(node), TextEnd::Enter | TextEnd::Blur) => {
            commit_rename(state, *node, &text);
        }
        (TextTarget::HierarchyRename(_), TextEnd::Escape) => cancel_rename(state),
        (TextTarget::HierarchyFilter, TextEnd::Escape) => state.hierarchy.filter.clear(),
        (TextTarget::PickerRoot, TextEnd::Enter) => picker::update(state, PickerMessage::Refresh),
        (TextTarget::PickerOpenPath, TextEnd::Enter) => {
            picker::update(state, PickerMessage::OpenTyped);
        }
        (TextTarget::PickerName, TextEnd::Enter) => picker::update(state, PickerMessage::Create),
        (TextTarget::Project(_), _) => project::text_ended(state, &target, &text, end),
        (TextTarget::Inspector(_), _) => inspector::text_ended(state, &target, &text, end),
        _ => {}
    }
}

/// A press in a field: it takes the keyboard when it has not got it, and puts
/// the caret before the character the press landed on.
pub fn press(state: &mut EditorState, target: &TextTarget, caret: usize) {
    if !state.text.has(target) {
        finish(state, TextEnd::Blur);
        let text = source_text(state, target);
        state.text.begin(target.clone(), &text);
    }
    state.text.edit.set_caret(caret, state.modifiers.shift);
}

/// A drag that began in the field extends the selection to a character.
pub fn drag(state: &mut EditorState, target: &TextTarget, caret: usize) {
    if state.text.has(target) {
        state.text.edit.set_caret(caret, true);
    }
}

/// A press that landed outside every field: the field that had the keyboard
/// keeps what was typed and lets go.
pub fn blur(state: &mut EditorState) {
    finish(state, TextEnd::Blur);
}

/// Gives a key to the field that has the keyboard; false when no field has it
/// or the field does not take the key, so a shortcut may.
pub fn key(state: &mut EditorState, event: &KeyEvent) -> bool {
    if state.text.target.is_none() {
        return false;
    }
    let outcome = state.text.edit.handle_key(event);
    if !outcome.handled {
        return false;
    }
    let mut changed = outcome.changed;
    match outcome.clipboard {
        Some(ClipboardRequest::Copy(text) | ClipboardRequest::Cut(text)) => {
            state.text.clipboard = text;
        }
        Some(ClipboardRequest::Paste) => {
            let pasted = state.text.clipboard.clone();
            changed |= state.text.edit.insert(&pasted);
        }
        None => {}
    }
    if changed {
        after_change(state);
    }
    if outcome.submitted {
        finish(state, TextEnd::Enter);
    } else if outcome.cancelled {
        finish(state, TextEnd::Escape);
    }
    true
}
