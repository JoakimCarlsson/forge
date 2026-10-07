//! The state of the editor: the open project, its documents, the preview and
//! viewport, the panels' own state and everything the interface shows.
//!
//! Panels read this and send messages; nothing in it knows a window or a
//! device.

use std::path::PathBuf;
use std::sync::Arc;

use fr_authoring::{DocumentId, OpenDocument, Workspace};
use fr_document::builtin_schemas;
use fr_input::Modifiers;
use fr_math::{Point, Rect};
use fr_project::Project;
use fr_ui::{DockDrag, DockTree, Rects, TextEdit};

use super::message::{MenuKind, TextTarget};
use super::picker::PickerState;
use super::prompt::Prompt;
use crate::panels::console::{ConsoleLevel, ConsoleState};
use crate::panels::hierarchy_state::HierarchyState;
use crate::panels::inspector::InspectorPanelState;
use crate::panels::project::ProjectPanelState;
use crate::panels::registry;
use crate::preview::Preview;
use crate::run::Runner;
use crate::settings::Settings;
use crate::subsystems::SubsystemRegistry;
use crate::viewport::Viewport;

/// The popup menu open over the window, at the point it was opened.
#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    /// Where the pointer was when the menu was opened.
    pub origin: Point,
    /// What the menu is for.
    pub kind: PopupKind,
}

/// What a popup menu belongs to.
#[derive(Clone, Debug, PartialEq)]
pub enum PopupKind {
    /// The hierarchy's menu, on a node or, without one, on the empty area.
    Hierarchy(Option<fr_document::Guid>),
}

/// The menu bar's state: which menu is open and which submenu is expanded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuState {
    /// The open menu.
    pub open: Option<MenuKind>,
    /// The label of the expanded submenu.
    pub submenu: Option<String>,
}

/// A text field that has the keyboard: what it edits and the edit itself.
#[derive(Debug, Default)]
pub struct TextFocus {
    /// What the field belongs to; none when no field has the keyboard.
    pub target: Option<TextTarget>,
    /// The line being edited.
    pub edit: TextEdit,
    /// What copy and cut put aside for paste.
    pub clipboard: String,
}

impl TextFocus {
    /// Gives the keyboard to a field showing `text`, all of it selected.
    pub fn begin(&mut self, target: TextTarget, text: &str) {
        self.target = Some(target);
        self.edit = TextEdit::selected(text);
    }

    /// Takes the keyboard from the field and returns what it was editing.
    pub fn end(&mut self) -> Option<(TextTarget, String)> {
        let target = self.target.take()?;
        Some((target, self.edit.text().to_owned()))
    }

    /// Whether the field of `target` has the keyboard.
    pub fn has(&self, target: &TextTarget) -> bool {
        self.target.as_ref() == Some(target)
    }
}

/// Everything the editor is showing and editing.
pub struct EditorState {
    /// The open project; none while the picker is shown.
    pub project: Option<Project>,
    /// The documents of the project that are open.
    pub workspace: Workspace,
    /// The per-user settings.
    pub settings: Settings,
    /// What each component type contributes to the editor.
    pub subsystems: SubsystemRegistry,
    /// The current document realized into a stage.
    pub preview: Preview,
    /// The camera, picking and gizmo over the preview.
    pub viewport: Viewport,
    /// The game process, when one is built or running.
    pub runner: Runner,
    /// The Console panel's log and view.
    pub console: ConsoleState,
    /// The layout of the panels.
    pub dock: DockTree,
    /// The tab being carried across the dock.
    pub dock_drag: Option<DockDrag>,
    /// Where the last frame painted the dock's groups and the panels' rows.
    pub rects: Rects,
    /// The menu bar.
    pub menu: MenuState,
    /// The popup menu, when one is open.
    pub popup: Option<Popup>,
    /// The text field with the keyboard.
    pub text: TextFocus,
    /// The Hierarchy panel's state.
    pub hierarchy: HierarchyState,
    /// The Project panel's state.
    pub project_panel: ProjectPanelState,
    /// The Inspector panel's state.
    pub inspector_panel: InspectorPanelState,
    /// The project picker, shown instead of the editor while it is set.
    pub picker: Option<PickerState>,
    /// The question about unsaved changes, while one is open.
    pub prompt: Option<Prompt>,
    /// The line the status bar shows for a while.
    pub toast: Option<String>,
    /// The panel the keyboard goes to.
    pub focused_panel: Option<String>,
    /// Where the 3D view is, as of the last frame.
    pub viewport_rect: Rect,
    /// The pointer as of the last event.
    pub pointer: Point,
    /// The modifier keys as of the last event.
    pub modifiers: Modifiers,
    /// Whether the dock layout changed and is not yet written.
    pub layout_dirty: bool,
    /// Whether the editor should end.
    pub quit: bool,
    /// Whether settings, layout and project state are written to disk; a
    /// headless capture leaves them alone.
    pub persist: bool,
}

impl EditorState {
    /// A state with no project open, the settings given and the default layout.
    pub fn new(settings: Settings) -> Self {
        let schemas = Arc::new(builtin_schemas());
        Self {
            project: None,
            workspace: Workspace::new(PathBuf::new(), schemas),
            settings,
            subsystems: SubsystemRegistry::builtin(),
            preview: Preview::new(&PathBuf::new()),
            viewport: Viewport::new(),
            runner: Runner::default(),
            console: ConsoleState::default(),
            dock: registry::default_layout(),
            dock_drag: None,
            rects: Rects::new(),
            menu: MenuState::default(),
            popup: None,
            text: TextFocus::default(),
            hierarchy: HierarchyState::default(),
            project_panel: ProjectPanelState::default(),
            inspector_panel: InspectorPanelState::default(),
            picker: None,
            prompt: None,
            toast: None,
            focused_panel: None,
            viewport_rect: Rect::default(),
            pointer: Point::default(),
            modifiers: Modifiers::default(),
            layout_dirty: false,
            quit: false,
            persist: true,
        }
    }

    /// Adds a line to the console.
    pub fn log(&mut self, level: ConsoleLevel, text: impl Into<String>) {
        self.console.push(level, text);
    }

    /// Adds an informational line to the console.
    pub fn info(&mut self, text: impl Into<String>) {
        self.log(ConsoleLevel::Info, text);
    }

    /// Adds an error line to the console and shows it in the status bar.
    pub fn error(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.toast = Some(text.clone());
        self.log(ConsoleLevel::Error, text);
    }

    /// The current document.
    pub fn document(&self) -> Option<&OpenDocument> {
        self.workspace.current()
    }

    /// The current document, to edit through commands.
    pub fn document_mut(&mut self) -> Option<&mut OpenDocument> {
        self.workspace.current_mut()
    }

    /// The handle of the current document.
    pub fn document_id(&self) -> Option<DocumentId> {
        self.workspace.current_id()
    }

    /// Writes the per-user settings when persistence is on, reporting a failure.
    pub fn persist_settings(&mut self) {
        if !self.persist {
            return;
        }
        if let Err(error) = self.settings.save() {
            self.error(format!("cannot save the settings: {error}"));
        }
    }

    /// Whether a project is open.
    pub fn has_project(&self) -> bool {
        self.project.is_some()
    }
}
