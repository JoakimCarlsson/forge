//! The panels of the editor: Hierarchy, Project, Inspector and Console, and the
//! registry that names them for the dock.
//!
//! A panel reads the [`EditorState`](crate::app::EditorState) and builds a
//! [`Div`](fr_ui::Div) of the app's [`Message`](crate::app::Message); what the
//! user does comes back as the panel's own message enum nested in `Message`,
//! and the panel's `update` turns it into `fr_authoring` commands or state
//! changes. A panel never mutates a document directly.
//!
//! Modules:
//!
//! - `registry`: [`PanelEntry`](registry::PanelEntry) and the panels' names
//! - `hierarchy`, `hierarchy_state`, `hierarchy_rows`, `hierarchy_update`,
//!   `hierarchy_menu`: the tree of the current document
//! - `project`, `project_cache`, `project_logic`, `project_view`: the files of
//!   the project
//! - `inspector`, `inspector_rows`, `inspector_scene`, `inspector_view`,
//!   `row_editor`, `rows`, `asset_slot`: the properties of the selection
//! - `text_end`: how typing in a panel's field ended
//! - `console`: the log

pub mod asset_slot;
pub mod console;
pub mod hierarchy;
pub mod hierarchy_menu;
pub mod hierarchy_rows;
pub mod hierarchy_state;
pub mod hierarchy_update;
pub mod inspector;
pub mod inspector_rows;
pub mod inspector_scene;
pub mod inspector_view;
pub mod project;
pub mod project_cache;
pub mod project_logic;
pub mod project_view;
pub mod registry;
pub mod row_editor;
pub mod rows;
pub mod text_end;
