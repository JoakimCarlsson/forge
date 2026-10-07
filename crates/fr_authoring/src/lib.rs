//! The headless authoring layer of the forge engine: what the editor and a
//! later command-line front end share.
//!
//! It owns the open document with its history and selection, the commands that
//! edit it, the editing rules over components that the schemas drive, the set of
//! open documents, the file operations of the Project panel and the planning of
//! a game launch. It depends only on the engine's data crates, and nothing here
//! knows a window, a device, a scene stage or the editor.
//!
//! Modules:
//!
//! - `document`, `data`, `kind`: the open [`Document`], its [`DocumentData`] and
//!   the [`DocumentKind`]
//! - `command`, `apply`, `tree`: the [`Command`]s and their execution over the
//!   hierarchy
//! - `history`, `selection`, `open_document`: undo and redo with gestures, the
//!   [`Selection`] and the [`OpenDocument`] that pairs them with a document
//! - `rules`: schema driven editing rules and inspector row data
//! - `workspace`: the [`Workspace`] of open documents
//! - `project_files`, `starter`: creating folders, scenes and prefabs
//! - `launch`: the [`LaunchPlan`] of a game
//! - `error`: [`AuthoringError`]
//!
//! The crate root is the facade and nothing else.

mod apply;
mod command;
mod data;
mod document;
mod error;
mod history;
mod kind;
mod launch;
mod open_document;
mod project_files;
mod rules;
mod selection;
mod starter;
mod tree;
mod workspace;

pub use command::{
    AddPrefabInstance, CoalesceKey, Command, CommandOutcome, ComponentInit, CreateEntity,
};
pub use data::{DocumentData, Snapshot};
pub use document::Document;
pub use error::AuthoringError;
pub use history::History;
pub use kind::DocumentKind;
pub use launch::{LaunchCommand, LaunchPlan, build_plan, launch_plan};
pub use open_document::OpenDocument;
pub use project_files::{
    CreatedAsset, SceneTemplate, asset_reference, create_folder, create_prefab_file,
    create_scene_file, model_entity_command, prefab_instance_command,
};
pub use rules::{
    AddPlan, ComponentChoice, ComponentSection, PropertyRow, addable_components,
    can_remove_component, component_sections, default_component, plan_add_component, property_rows,
    removal_blockers, validate_property_value,
};
pub use selection::Selection;
pub use starter::{empty_scene, starter_prefab, starter_scene};
pub use tree::{node_kind, sibling_index, subtree_ids};
pub use workspace::{DocumentId, SaveResult, Workspace};
