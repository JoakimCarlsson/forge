//! What the editor's actions do, from a menu, a shortcut or a button.

use std::path::Path;

use fr_authoring::DocumentKind;

use super::documents::{close_now, new_document, save_all, save_current, scene_to_play};
use super::edit;
use super::layout::{reset_layout, toggle_panel};
use super::message::Action;
use super::picker::open_now;
use super::project_open::close_project;
use super::prompt::{Pending, Prompt};
use super::state::EditorState;
use crate::panels::console::ConsoleLevel;
use crate::run::{RunEvent, RunState, Stream};

/// Puts what a build or a game produced into the console.
pub fn absorb(state: &mut EditorState, events: Vec<RunEvent>) {
    for event in events {
        match event {
            RunEvent::Output(line) => {
                let level = match line.stream {
                    Stream::Editor => ConsoleLevel::Info,
                    Stream::Stdout | Stream::Stderr => ConsoleLevel::Game,
                };
                state.log(level, line.text);
            }
            RunEvent::Exited { .. } | RunEvent::BuildFinished { .. } => {}
        }
    }
}

/// Saves everything and starts a build, with or without running the game.
fn play(state: &mut EditorState, build_first: bool, run: bool) {
    let Some(root) = state
        .project
        .as_ref()
        .map(|project| project.root().to_path_buf())
    else {
        return;
    };
    let scene = if run {
        match scene_to_play(state) {
            Ok(scene) => Some(scene),
            Err(error) => {
                state.error(error);
                return;
            }
        }
    } else {
        None
    };
    let saved = save_all(state);
    let started = match &scene {
        Some(scene) => state
            .runner
            .play(&root, Path::new(scene), build_first, saved),
        None => state.runner.build(&root, saved),
    };
    match started {
        Ok(()) => state.info(if run { "playing" } else { "building" }),
        Err(error) => state.error(error),
    }
}

/// Stops the build or the game.
fn stop(state: &mut EditorState) {
    let events = state.runner.stop();
    absorb(state, events);
}

/// Asks about unsaved changes before leaving, or goes on when there are none.
fn confirm_or(state: &mut EditorState, text: &str, then: Pending) {
    if state.workspace.dirty_documents().is_empty() {
        proceed(state, then);
    } else {
        state.prompt = Some(Prompt {
            text: text.to_owned(),
            then,
        });
    }
}

/// Does what a question about unsaved changes was holding back.
pub fn proceed(state: &mut EditorState, pending: Pending) {
    match pending {
        Pending::CloseDocument(id) => close_now(state, id),
        Pending::CloseProject => close_project(state),
        Pending::Quit => {
            if state.runner.state() != RunState::Idle {
                stop(state);
            }
            state.quit = true;
        }
        Pending::OpenProject(path) => open_now(state, &path),
    }
}

/// Prints what the project file says.
fn show_project_info(state: &mut EditorState) {
    let Some(project) = &state.project else {
        return;
    };
    let settings = project.settings();
    let lines = [
        format!("project {}", settings.name),
        format!("  folder {}", project.root().display()),
        format!("  assets {}", project.asset_root().display()),
        format!(
            "  startup scene {}",
            project.startup_scene().unwrap_or("none")
        ),
        format!(
            "  game {}",
            project
                .game_binary()
                .map_or_else(|| "none".to_owned(), |path| path.display().to_string())
        ),
    ];
    for line in lines {
        state.info(line);
    }
}

/// Does an action.
pub fn perform(state: &mut EditorState, action: Action) {
    match action {
        Action::NewScene => new_document(state, DocumentKind::Scene),
        Action::NewPrefab => new_document(state, DocumentKind::Prefab),
        Action::OpenProject => state.show_picker(),
        Action::OpenRecent(index) => {
            let path = state
                .settings
                .recent_projects()
                .get(index)
                .map(|path| path.to_path_buf());
            if let Some(path) = path {
                confirm_or(
                    state,
                    "Save changes before opening another project?",
                    Pending::OpenProject(path),
                );
            }
        }
        Action::Save => save_current(state),
        Action::SaveAll => {
            let _ = save_all(state);
        }
        Action::CloseProject => confirm_or(
            state,
            "Save changes before closing the project?",
            Pending::CloseProject,
        ),
        Action::Quit => confirm_or(state, "Save changes before quitting?", Pending::Quit),
        Action::Undo => edit::undo(state),
        Action::Redo => edit::redo(state),
        Action::Delete => edit::delete_selection(state),
        Action::Duplicate => edit::duplicate_selection(state),
        Action::Rename => edit::begin_rename(state),
        Action::Build => play(state, false, false),
        Action::Play { build_first } => play(state, build_first, true),
        Action::Stop => stop(state),
        Action::FrameSelection => {
            let document = state.workspace.current();
            state.viewport.frame_selection(document, &state.preview);
        }
        Action::SetTool(tool) => state.viewport.set_tool(tool),
        Action::TogglePanel(id) => toggle_panel(state, id),
        Action::ResetLayout => reset_layout(state),
        Action::ShowProjectInfo => show_project_info(state),
        Action::RevealAssetRoot => {
            let root = state.workspace.asset_root().display().to_string();
            state.info(format!("asset root {root}"));
        }
    }
}
