//! Applying a message to the state.

use super::actions::perform;
use super::documents::{request_close, save_all};
use super::layout::apply_dock_event;
use super::message::{Message, PromptAnswer, TextMessage};
use super::picker;
use super::prompt::Pending;
use super::state::EditorState;
use super::text_input;
use crate::panels::{console, hierarchy_update, inspector, project};

/// Closes the menu bar's menus and the popup.
pub fn close_menus(state: &mut EditorState) {
    state.menu.open = None;
    state.menu.submenu = None;
    state.popup = None;
}

/// Answers the question about unsaved changes.
fn answer_prompt(state: &mut EditorState, answer: PromptAnswer) {
    let Some(prompt) = state.prompt.take() else {
        return;
    };
    match answer {
        PromptAnswer::Cancel => {}
        PromptAnswer::Discard => super::actions::proceed(state, prompt.then),
        PromptAnswer::Save => {
            let saved = match &prompt.then {
                Pending::CloseDocument(id) => state
                    .workspace
                    .save(*id)
                    .map_err(|error| error.to_string())
                    .inspect_err(|error| state.error(format!("save failed: {error}"))),
                _ => save_all(state),
            };
            if saved.is_ok() {
                super::actions::proceed(state, prompt.then);
            }
        }
    }
}

/// Applies a message.
pub fn update(state: &mut EditorState, message: Message) {
    match message {
        Message::Noop => {}
        Message::Action(action) => {
            close_menus(state);
            perform(state, action);
        }
        Message::ToggleMenu(kind) => {
            state.popup = None;
            state.menu.submenu = None;
            state.menu.open = if state.menu.open == Some(kind) {
                None
            } else {
                Some(kind)
            };
        }
        Message::CloseMenus => close_menus(state),
        Message::ToggleSubmenu(label) => {
            state.menu.submenu = if state.menu.submenu.as_deref() == Some(&label) {
                None
            } else {
                Some(label)
            };
        }
        Message::Dock(event) => apply_dock_event(state, event),
        Message::SelectDocument(id) => {
            let _ = state.workspace.set_current(id);
            super::project_open::save_project_state(state);
        }
        Message::CloseDocument(id) => request_close(state, id),
        Message::Hierarchy(message) => hierarchy_update::update(state, message),
        Message::Project(message) => project::update(state, message),
        Message::Inspector(message) => inspector::update(state, message),
        Message::Console(message) => console::update(state, message),
        Message::Picker(message) => picker::update(state, message),
        Message::Prompt(answer) => answer_prompt(state, answer),
        Message::Text(TextMessage::Press { target, caret }) => {
            text_input::press(state, &target, caret);
        }
        Message::Text(TextMessage::Drag { target, caret }) => {
            text_input::drag(state, &target, caret);
        }
    }
}
