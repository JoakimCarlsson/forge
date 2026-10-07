//! The application: state, messages, the loop, the menus and the layout.
//!
//! [`Editor`] is the whole app behind a handful of input methods and `frame`;
//! the window host and the headless capture drive the same one. The root view
//! is rebuilt from [`EditorState`] every frame and what the user does comes
//! back as a [`Message`] that `update` applies.
//!
//! Modules:
//!
//! - `state`, `message`, `prompt`: the state, the messages and the unsaved
//!   changes question
//! - `edit`, `fields`: running commands on the current document and text fields
//! - `project_open`, `documents`: opening projects and documents
//! - `actions`, `shortcuts`, `update`, `text_input`: what messages, keys and
//!   typing do
//! - `menus`, `layout`, `view`, `picker`, `status`: the interface
//! - `editor`, `feed`, `host`, `capture`: the frame loop, the viewport's input,
//!   the window handler and the headless capture

pub mod actions;
pub mod capture;
pub mod documents;
pub mod edit;
pub mod editor;
pub mod feed;
pub mod fields;
pub mod host;
pub mod layout;
pub mod menus;
pub mod message;
pub mod picker;
pub mod project_open;
pub mod prompt;
pub mod shortcuts;
pub mod state;
pub mod status;
pub mod text_input;
pub mod update;
pub mod view;

pub use editor::Editor;
pub use message::{
    Action, MenuKind, Message, PickerMessage, PromptAnswer, TextMessage, TextTarget,
};
pub use state::{EditorState, MenuState, Popup, PopupKind, TextFocus};
