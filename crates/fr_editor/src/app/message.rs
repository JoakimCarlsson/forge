//! The messages of the editor: everything the interface can ask for.
//!
//! A panel has its own message enum nested here, so a panel is written without
//! touching the others; the menus, the shortcuts and the Play wiring all speak
//! [`Action`].

use std::path::PathBuf;

use fr_authoring::DocumentId;
use fr_document::Guid;
use fr_ui::DockEvent;

use crate::panels::console::ConsoleMessage;
use crate::panels::hierarchy::HierarchyMessage;
use crate::panels::inspector::InspectorMessage;
use crate::panels::project::ProjectMessage;
use crate::viewport::Tool;

/// A menu of the menu bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    /// File.
    File,
    /// Edit.
    Edit,
    /// Project.
    Project,
    /// Play.
    Play,
    /// View.
    View,
}

/// A thing the user can ask the editor to do, from a menu, a shortcut or a
/// button.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Creates an unsaved scene.
    NewScene,
    /// Creates an unsaved prefab.
    NewPrefab,
    /// Shows the project picker.
    OpenProject,
    /// Opens a project of the recent list, by position.
    OpenRecent(usize),
    /// Saves the current document.
    Save,
    /// Saves every document with changes.
    SaveAll,
    /// Closes the project, asking about unsaved changes first.
    CloseProject,
    /// Ends the editor, asking about unsaved changes first.
    Quit,
    /// Undoes the last edit of the current document.
    Undo,
    /// Redoes the last undone edit.
    Redo,
    /// Deletes the selected nodes.
    Delete,
    /// Duplicates the selected nodes.
    Duplicate,
    /// Renames the primary selected node in the Hierarchy.
    Rename,
    /// Builds the project's game.
    Build,
    /// Saves everything and runs the game, building first when asked.
    Play {
        /// Whether the build runs before the game starts.
        build_first: bool,
    },
    /// Stops the build or the game.
    Stop,
    /// Frames the selection in the viewport.
    FrameSelection,
    /// Chooses the viewport's tool.
    SetTool(Tool),
    /// Shows or hides a panel.
    TogglePanel(&'static str),
    /// Puts the panels back where they start.
    ResetLayout,
    /// Shows the project's settings in the console.
    ShowProjectInfo,
    /// Prints where the project's assets live in the console.
    RevealAssetRoot,
}

/// What a text field belongs to, which decides what its edits do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextTarget {
    /// The filter at the top of the Hierarchy.
    HierarchyFilter,
    /// The name of a node being typed over in the Hierarchy.
    HierarchyRename(Guid),
    /// The folder the picker lists projects of.
    PickerRoot,
    /// The path typed into the picker to open.
    PickerOpenPath,
    /// The name of the project the picker creates.
    PickerName,
    /// A field of the Project panel, named by the panel.
    Project(String),
    /// A field of the Inspector panel, named by the panel.
    Inspector(String),
}

/// What happened to a text field.
#[derive(Clone, Debug, PartialEq)]
pub enum TextMessage {
    /// A press landed in a field at a character position.
    Press {
        /// The field.
        target: TextTarget,
        /// The character it landed before.
        caret: usize,
    },
    /// A drag that began in the field reached a character position.
    Drag {
        /// The field.
        target: TextTarget,
        /// The character it is now over.
        caret: usize,
    },
}

/// A button of the picker.
#[derive(Clone, Debug, PartialEq)]
pub enum PickerMessage {
    /// Opens a project folder.
    Open(PathBuf),
    /// Opens the folder typed into the open field.
    OpenTyped,
    /// Lists the projects of the typed folder again.
    Refresh,
    /// Creates the project named in the name field.
    Create,
    /// Leaves the picker for the project that was open.
    Cancel,
}

/// The answer to the question about unsaved changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptAnswer {
    /// Save, then go on.
    Save,
    /// Go on and lose the changes.
    Discard,
    /// Do not go on.
    Cancel,
}

/// Everything the interface can ask for.
#[derive(Clone, Debug)]
pub enum Message {
    /// Nothing.
    Noop,
    /// Does something.
    Action(Action),
    /// Opens a menu of the bar, or closes it when it is the open one.
    ToggleMenu(MenuKind),
    /// Closes the menus and the popup.
    CloseMenus,
    /// Expands a submenu by its label, or collapses it.
    ToggleSubmenu(String),
    /// Something the user did to the dock.
    Dock(DockEvent),
    /// Shows a document.
    SelectDocument(DocumentId),
    /// Closes a document, asking first when it has changes.
    CloseDocument(DocumentId),
    /// The Hierarchy's own messages.
    Hierarchy(HierarchyMessage),
    /// The Project panel's own messages.
    Project(ProjectMessage),
    /// The Inspector panel's own messages.
    Inspector(InspectorMessage),
    /// The Console's own messages.
    Console(ConsoleMessage),
    /// The picker's buttons.
    Picker(PickerMessage),
    /// The answer to the unsaved changes question.
    Prompt(PromptAnswer),
    /// A press or drag in a text field.
    Text(TextMessage),
}

impl Message {
    /// The message that does an action.
    pub fn action(action: Action) -> Self {
        Self::Action(action)
    }
}
