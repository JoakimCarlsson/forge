//! The Project panel: the asset root as a folder tree beside the files of the
//! selected folder.
//!
//! Clicking selects, double-clicking opens a scene or prefab as the current
//! document or adds a model to it as one undo entry, and the context menu and
//! the Create button make folders, scenes and prefabs through an inline name
//! prompt. The listing is cached in a [`ProjectCache`] and read again only
//! after Refresh or a creation. Search and the name prompt type through the
//! app's text focus under [`TextTarget::Project`] names: the app asks
//! [`text_source`] what a field starts with and calls [`text_ended`] when it
//! stops being typed into.
//!
//! Modules:
//!
//! - [`project_logic`](super::project_logic): paths, rows and file creation
//! - [`project_view`](super::project_view): the layout
//! - [`project_cache`](super::project_cache): the cached listing

use std::collections::HashSet;
use std::rc::Rc;

use fr_authoring::Command;
use fr_document::{AssetKind, Guid};
use fr_project::Project;
use fr_ui::{Div, Point, Scroll, ScrollEvent, Theme};

use super::project_cache::{ProjectCache, ProjectListing};
use super::project_logic::{CreateKind, absolute_of, create_entry, opens_as_document, parent_of};
use super::project_view;
use super::text_end::TextEnd;
use crate::app::{EditorState, Message, TextTarget, documents, edit};

/// The name of the search field's text focus target.
const SEARCH_FIELD: &str = "search";

/// The name of the name prompt's text focus target.
const NAME_FIELD: &str = "name";

/// What a context menu was opened on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuTarget {
    /// A folder, by its path relative to the asset root.
    Folder(String),
    /// A file, by its path relative to the asset root.
    File(String),
    /// The empty part of the file list.
    Background,
}

/// What the user did in the Project panel.
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectMessage {
    /// A folder of the tree or list was clicked.
    SelectFolder(String),
    /// A folder's chevron was clicked.
    ToggleFolder(String),
    /// A file of the list was clicked.
    SelectFile(String),
    /// An entry of the list was double-clicked, or Open was chosen.
    Open(String),
    /// The secondary button was pressed on an entry.
    OpenMenu(MenuTarget),
    /// A press landed outside the open menu.
    CloseMenu,
    /// The Create button was clicked.
    ToggleCreateMenu,
    /// A kind of entry was chosen to create: ask for its name.
    BeginCreate(CreateKind),
    /// The name was confirmed.
    ConfirmCreate,
    /// The name prompt was dismissed.
    CancelCreate,
    /// Refresh was chosen.
    Refresh,
    /// The search's clear button was clicked.
    ClearSearch,
    /// The folder tree was scrolled.
    ScrollTree(ScrollEvent),
    /// The file list was scrolled.
    ScrollList(ScrollEvent),
}

/// A context menu and where it was opened.
#[derive(Clone, Debug)]
pub struct ProjectMenu {
    /// The pointer when the menu was opened.
    pub origin: Point,
    /// What it was opened on.
    pub target: MenuTarget,
}

/// The question for the name of a new entry.
#[derive(Clone, Debug)]
pub struct CreatePrompt {
    /// What is being created.
    pub kind: CreateKind,
    /// The folder it goes in, relative to the asset root.
    pub folder: String,
    /// The name typed so far, kept while the field does not have the keyboard.
    pub name: String,
}

/// The Project panel's own state, kept in the [`EditorState`].
#[derive(Debug, Default)]
pub struct ProjectPanelState {
    /// The cached listing of the asset root.
    cache: ProjectCache,
    /// The folder whose files the list shows; empty at the asset root.
    pub(super) folder: String,
    /// The selected entry of the list.
    pub(super) file: Option<String>,
    /// The folders of the tree that show their children.
    pub(super) expanded: HashSet<String>,
    /// The context menu, when open.
    pub(super) menu: Option<ProjectMenu>,
    /// Whether the Create menu is open.
    pub(super) create_menu: bool,
    /// The name prompt, when open.
    pub(super) prompt: Option<CreatePrompt>,
    /// The search as last typed, kept while the field does not have the
    /// keyboard.
    pub(super) filter: String,
    /// How far the tree is scrolled.
    pub(super) tree_scroll: Scroll,
    /// How far the list is scrolled.
    pub(super) list_scroll: Scroll,
}

impl ProjectPanelState {
    /// The listing of a project's asset root, read on first use and kept until
    /// [`ProjectPanelState::invalidate`]; empty without a project.
    pub fn listing(&self, project: Option<&Project>) -> Rc<ProjectListing> {
        match project {
            Some(project) => self.cache.get(project),
            None => Rc::default(),
        }
    }

    /// Drops the cached listing, as when files were saved or changed outside
    /// the panel.
    pub fn invalidate(&self) {
        self.cache.invalidate();
    }
}

/// The text focus target of the search field.
pub fn search_target() -> TextTarget {
    TextTarget::Project(SEARCH_FIELD.to_owned())
}

/// The text focus target of the name prompt's field.
pub fn name_target() -> TextTarget {
    TextTarget::Project(NAME_FIELD.to_owned())
}

/// The panel's contents.
pub fn view(state: &EditorState, theme: &Theme) -> Div<Message> {
    project_view::view(state, theme)
}

/// The text a field of this panel starts with when it is given the keyboard.
pub fn text_source(state: &EditorState, target: &TextTarget) -> String {
    let panel = &state.project_panel;
    if *target == search_target() {
        panel.filter.clone()
    } else if *target == name_target() {
        panel
            .prompt
            .as_ref()
            .map(|prompt| prompt.name.clone())
            .unwrap_or_default()
    } else {
        String::new()
    }
}

/// A field of this panel stopped being typed into. Search keeps what was typed
/// and Escape clears it; the name is kept, Enter confirms it and Escape
/// dismisses the prompt.
pub fn text_ended(state: &mut EditorState, target: &TextTarget, text: &str, end: TextEnd) {
    if *target == search_target() {
        state.project_panel.filter = if end.commits() {
            text.to_owned()
        } else {
            String::new()
        };
    } else if *target == name_target() {
        match end {
            TextEnd::Enter => {
                set_prompt_name(state, text);
                confirm_create(state);
            }
            TextEnd::Blur => set_prompt_name(state, text),
            TextEnd::Escape => state.project_panel.prompt = None,
        }
    }
}

/// Stores the name typed into the prompt.
fn set_prompt_name(state: &mut EditorState, text: &str) {
    if let Some(prompt) = &mut state.project_panel.prompt {
        prompt.name = text.to_owned();
    }
}

/// Applies a message of the panel.
pub fn update(state: &mut EditorState, message: ProjectMessage) {
    let panel = &mut state.project_panel;
    match message {
        ProjectMessage::SelectFolder(folder) => {
            panel.folder = folder;
            panel.file = None;
            panel.menu = None;
            panel.create_menu = false;
        }
        ProjectMessage::ToggleFolder(folder) => {
            if !panel.expanded.remove(&folder) {
                panel.expanded.insert(folder);
            }
        }
        ProjectMessage::SelectFile(path) => panel.file = Some(path),
        ProjectMessage::Open(path) => open_entry(state, &path),
        ProjectMessage::OpenMenu(target) => open_menu(state, target),
        ProjectMessage::CloseMenu => {
            panel.menu = None;
            panel.create_menu = false;
        }
        ProjectMessage::ToggleCreateMenu => {
            panel.menu = None;
            panel.create_menu = !panel.create_menu;
        }
        ProjectMessage::BeginCreate(kind) => begin_create(state, kind),
        ProjectMessage::ConfirmCreate => confirm_create(state),
        ProjectMessage::CancelCreate => cancel_create(state),
        ProjectMessage::Refresh => {
            panel.menu = None;
            panel.cache.invalidate();
        }
        ProjectMessage::ClearSearch => {
            panel.filter.clear();
            if state.text.has(&search_target()) {
                state.text.end();
            }
        }
        ProjectMessage::ScrollTree(event) => panel.tree_scroll.apply(event),
        ProjectMessage::ScrollList(event) => panel.list_scroll.apply(event),
    }
}

/// Closes the menu the panel has open, as Escape does; false when nothing was
/// open.
pub fn dismiss(state: &mut EditorState) -> bool {
    let panel = &mut state.project_panel;
    let open = panel.menu.is_some() || panel.create_menu;
    panel.menu = None;
    panel.create_menu = false;
    open
}

/// Opens the context menu at the pointer and selects what it is on.
fn open_menu(state: &mut EditorState, target: MenuTarget) {
    let origin = state.pointer;
    let panel = &mut state.project_panel;
    match &target {
        MenuTarget::Folder(folder) => panel.folder.clone_from(folder),
        MenuTarget::File(path) => panel.file = Some(path.clone()),
        MenuTarget::Background => {}
    }
    panel.create_menu = false;
    panel.menu = Some(ProjectMenu { origin, target });
}

/// Opens a folder, a scene or a prefab, or adds a model to the current document.
fn open_entry(state: &mut EditorState, relative: &str) {
    let Some(root) = state.project.as_ref().map(Project::asset_root) else {
        return;
    };
    let absolute = absolute_of(&root, relative);
    state.project_panel.menu = None;
    if absolute.is_dir() {
        let panel = &mut state.project_panel;
        panel.folder = relative.to_owned();
        panel.file = None;
        panel.expanded.insert(relative.to_owned());
        return;
    }
    state.project_panel.file = Some(relative.to_owned());
    match AssetKind::of_path(relative) {
        kind if opens_as_document(kind) => documents::open_path(state, &absolute),
        AssetKind::Model => add_model(state, relative),
        _ => state.info(format!("{relative} cannot be opened in the editor")),
    }
}

/// Adds a mesh renderer entity for a model to the current document as one undo
/// entry and selects it.
fn add_model(state: &mut EditorState, relative: &str) {
    let Some(open) = state.document() else {
        state.error("open a scene or prefab to add a model to");
        return;
    };
    let asset_root = open.document().asset_root().to_path_buf();
    let model = absolute_of(&asset_root, relative);
    match fr_authoring::model_entity_command(&asset_root, &model, Guid::NONE) {
        Ok(create) => {
            if let Some(outcome) = edit::run(state, &Command::CreateEntity(create)) {
                edit::select(state, &outcome.created);
            }
        }
        Err(error) => state.error(format!("cannot add {relative}: {error}")),
    }
}

/// Opens the name prompt for a new entry in the selected folder.
fn begin_create(state: &mut EditorState, kind: CreateKind) {
    let panel = &mut state.project_panel;
    panel.menu = None;
    panel.create_menu = false;
    panel.prompt = Some(CreatePrompt {
        kind,
        folder: panel.folder.clone(),
        name: kind.default_name().to_owned(),
    });
    state.text.begin(name_target(), kind.default_name());
}

/// Closes the name prompt.
fn cancel_create(state: &mut EditorState) {
    state.project_panel.prompt = None;
    if state.text.has(&name_target()) {
        state.text.end();
    }
}

/// Creates what the prompt asks for, refreshes the listing and opens a new
/// scene or prefab; a refusal is reported and the prompt stays open.
fn confirm_create(state: &mut EditorState) {
    let Some(prompt) = state.project_panel.prompt.clone() else {
        return;
    };
    let name = if state.text.has(&name_target()) {
        state.text.edit.text().to_owned()
    } else {
        prompt.name.clone()
    };
    let Some(root) = state.project.as_ref().map(Project::asset_root) else {
        return;
    };
    match create_entry(&root, &prompt.folder, prompt.kind, &name) {
        Ok(path) => {
            cancel_create(state);
            created(state, &prompt, &path);
        }
        Err(error) => state.error(format!(
            "cannot create {}: {error}",
            prompt.kind.label().to_lowercase()
        )),
    }
}

/// Shows a created entry: refreshes the listing, expands and selects its
/// folder and opens a scene or prefab.
fn created(state: &mut EditorState, prompt: &CreatePrompt, path: &std::path::Path) {
    state.info(format!("created {}", path.display()));
    let panel = &mut state.project_panel;
    panel.cache.invalidate();
    let mut folder = prompt.folder.clone();
    while !folder.is_empty() {
        panel.expanded.insert(folder.clone());
        folder = parent_of(&folder).to_owned();
    }
    panel.folder.clone_from(&prompt.folder);
    if prompt.kind != CreateKind::Folder {
        documents::open_path(state, path);
    }
}
