//! The project picker: the screen shown when no project is open.
//!
//! A centred card lists the recent projects and the projects found under the
//! projects folder, opens a folder typed in, and creates a new project.

use std::path::{Path, PathBuf};

use fr_ui::{
    Div, IconName, IconSize, Styled, Theme, button, div, h_flex, icon, rule, text, v_flex,
};

use super::fields::text_input;
use super::message::{Message, PickerMessage, TextTarget};
use super::project_open::{new_project, open_project};
use super::prompt::{Pending, Prompt};
use super::state::EditorState;
use crate::settings::{DiscoveredProject, discover_projects};

/// The most rows each list of the picker shows.
const MAX_ROWS: usize = 6;

/// The width of the card, in logical pixels.
const CARD_WIDTH: f32 = 560.0;

/// What the picker is showing and what has been typed into it.
#[derive(Clone, Debug, Default)]
pub struct PickerState {
    /// The folder projects are listed from and created in.
    pub root: String,
    /// The path typed to open.
    pub open_path: String,
    /// The name typed for a new project.
    pub name: String,
    /// The projects found under `root`.
    pub discovered: Vec<DiscoveredProject>,
    /// What went wrong the last time, when something did.
    pub error: Option<String>,
}

impl PickerState {
    /// The text of one of the picker's fields.
    pub fn field_text(&self, target: &TextTarget) -> Option<&str> {
        match target {
            TextTarget::PickerRoot => Some(&self.root),
            TextTarget::PickerOpenPath => Some(&self.open_path),
            TextTarget::PickerName => Some(&self.name),
            _ => None,
        }
    }

    /// Replaces the text of one of the picker's fields.
    pub fn set_field_text(&mut self, target: &TextTarget, value: &str) {
        match target {
            TextTarget::PickerRoot => value.clone_into(&mut self.root),
            TextTarget::PickerOpenPath => value.clone_into(&mut self.open_path),
            TextTarget::PickerName => value.clone_into(&mut self.name),
            _ => {}
        }
    }
}

impl EditorState {
    /// Shows the picker, listing the projects of the settings' folder.
    pub fn show_picker(&mut self) {
        let root = self.settings.projects_root().to_path_buf();
        self.picker = Some(PickerState {
            root: root.display().to_string(),
            discovered: discover_projects(&root),
            ..PickerState::default()
        });
    }
}

/// One project as a row: its name over its path.
fn project_row(theme: &Theme, name: &str, path: &Path) -> Div<Message> {
    h_flex()
        .w_full()
        .px(3)
        .py(1.5)
        .gap(3)
        .items_center()
        .rounded(theme.radius.md)
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::Picker(PickerMessage::Open(path.to_path_buf())))
        .child(
            icon(IconName::FolderOpen)
                .size(IconSize::Medium)
                .color(theme.colors.text_muted),
        )
        .child(
            v_flex()
                .gap(0.5)
                .child(text(name.to_owned()).text_sm().font_medium())
                .child(
                    text(path.display().to_string())
                        .text_xs()
                        .color(theme.colors.text_subtle),
                ),
        )
}

/// A heading above a list.
fn heading(theme: &Theme, label: &str) -> Div<Message> {
    div().w_full().child(
        text(label.to_owned())
            .text_xs()
            .font_semibold()
            .color(theme.colors.text_muted),
    )
}

/// The directory name of a path.
fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The recent projects section.
fn recent_section(state: &EditorState, theme: &Theme) -> Div<Message> {
    let recent = state.settings.recent_projects();
    let mut section = v_flex()
        .w_full()
        .gap(1)
        .child(heading(theme, "RECENT PROJECTS"));
    if recent.is_empty() {
        section = section.child(
            text("No recent projects")
                .text_sm()
                .color(theme.colors.text_subtle),
        );
    }
    for path in recent.into_iter().take(MAX_ROWS) {
        section = section.child(project_row(theme, &folder_name(path), path));
    }
    section
}

/// The discovered projects section with its folder field and Refresh.
fn discovered_section(state: &EditorState, theme: &Theme, picker: &PickerState) -> Div<Message> {
    let folder = h_flex()
        .w_full()
        .gap(2)
        .items_center()
        .child(
            div()
                .flex_1()
                .child(text_input(state, &TextTarget::PickerRoot, &picker.root)),
        )
        .child(button("Refresh", Message::Picker(PickerMessage::Refresh)));
    let mut section = v_flex()
        .w_full()
        .gap(1)
        .child(heading(theme, "PROJECTS FOLDER"))
        .child(folder);
    if picker.discovered.is_empty() {
        section = section.child(
            text("No projects in this folder")
                .text_sm()
                .color(theme.colors.text_subtle),
        );
    }
    for project in picker.discovered.iter().take(MAX_ROWS) {
        section = section.child(project_row(theme, &project.name, &project.path));
    }
    section
}

/// A labelled field with a button beside it.
fn field_with_button(
    state: &EditorState,
    theme: &Theme,
    label: &str,
    target: &TextTarget,
    value: &str,
    action: &str,
    message: PickerMessage,
) -> Div<Message> {
    v_flex().w_full().gap(1).child(heading(theme, label)).child(
        h_flex()
            .w_full()
            .gap(2)
            .items_center()
            .child(div().flex_1().child(text_input(state, target, value)))
            .child(button(action.to_owned(), Message::Picker(message)).filled()),
    )
}

/// The picker screen.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    let Some(picker) = &state.picker else {
        return div();
    };
    let mut card = v_flex()
        .w_px(CARD_WIDTH)
        .p(6)
        .gap(4)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .rounded(theme.radius.lg)
        .child(text("Forge").text_xxl().font_semibold())
        .child(recent_section(state, theme))
        .child(rule(theme))
        .child(discovered_section(state, theme, picker))
        .child(rule(theme))
        .child(field_with_button(
            state,
            theme,
            "OPEN A FOLDER",
            &TextTarget::PickerOpenPath,
            &picker.open_path,
            "Open",
            PickerMessage::OpenTyped,
        ))
        .child(field_with_button(
            state,
            theme,
            "NEW PROJECT",
            &TextTarget::PickerName,
            &picker.name,
            "New Project",
            PickerMessage::Create,
        ));
    if let Some(error) = &picker.error {
        card = card.child(text(error.clone()).text_sm().color(theme.colors.danger));
    }
    if state.has_project() {
        card = card.child(
            button(
                "Back to the project",
                Message::Picker(PickerMessage::Cancel),
            )
            .ghost(),
        );
    }
    v_flex()
        .w_full()
        .h_full()
        .bg(theme.colors.background)
        .items_center()
        .justify_center()
        .child(card)
}

/// Opens a project, asking first when the one open has unsaved changes; a
/// failure is shown in the picker.
fn request_open(state: &mut EditorState, path: PathBuf) {
    if !state.workspace.dirty_documents().is_empty() {
        state.prompt = Some(Prompt {
            text: "Save changes before opening another project?".to_owned(),
            then: Pending::OpenProject(path),
        });
        return;
    }
    open_now(state, &path);
}

/// Opens a project without asking, showing a failure in the picker.
pub fn open_now(state: &mut EditorState, path: &Path) {
    if let Err(error) = open_project(state, path) {
        state.error(error.clone());
        if let Some(picker) = &mut state.picker {
            picker.error = Some(error);
        }
    }
}

/// Applies a button of the picker.
pub fn update(state: &mut EditorState, message: PickerMessage) {
    match message {
        PickerMessage::Open(path) => request_open(state, path),
        PickerMessage::OpenTyped => {
            let typed = state
                .picker
                .as_ref()
                .map(|picker| PathBuf::from(picker.open_path.trim()));
            if let Some(path) = typed {
                request_open(state, path);
            }
        }
        PickerMessage::Refresh => {
            let root = state
                .picker
                .as_ref()
                .map(|picker| PathBuf::from(picker.root.trim()));
            if let Some(root) = root {
                state.settings.set_projects_root(&root);
                if let Some(picker) = &mut state.picker {
                    picker.discovered = discover_projects(&root);
                    picker.error = None;
                }
            }
        }
        PickerMessage::Create => {
            let name = state
                .picker
                .as_ref()
                .map(|picker| picker.name.clone())
                .unwrap_or_default();
            if let Err(error) = new_project(state, &name) {
                state.error(error.clone());
                if let Some(picker) = &mut state.picker {
                    picker.error = Some(error);
                }
            }
        }
        PickerMessage::Cancel => state.picker = None,
    }
}
