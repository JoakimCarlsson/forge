//! The status bar along the bottom of the window.

use fr_ui::{Div, Theme, h_flex, status_bar, text};

use super::message::Message;
use super::state::EditorState;
use crate::run::RunState;

/// A line of small muted text.
fn caption(theme: &Theme, content: impl Into<String>) -> Div<Message> {
    h_flex().child(text(content).text_xs().color(theme.colors.text_muted))
}

/// The left side: the last note, or what the runner is doing.
fn start(state: &EditorState, theme: &Theme) -> Div<Message> {
    let note = match state.runner.state() {
        RunState::Building => "Building...".to_owned(),
        RunState::Running => "Playing".to_owned(),
        RunState::Idle => state.toast.clone().unwrap_or_else(|| "Ready".to_owned()),
    };
    caption(theme, note)
}

/// The right side: the project, the document and the selection.
fn end(state: &EditorState, theme: &Theme) -> Div<Message> {
    let mut parts = Vec::new();
    if let Some(open) = state.document() {
        let selected = open.selection().len();
        if selected > 0 {
            parts.push(format!("{selected} selected"));
        }
        parts.push(open.document().relative_path().to_owned());
    }
    if let Some(project) = &state.project {
        parts.push(project.settings().name.clone());
    }
    caption(theme, parts.join("   "))
}

/// The status bar.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    status_bar(theme, start(state, theme), end(state, theme))
}
