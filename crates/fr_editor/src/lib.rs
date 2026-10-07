//! The forge editor: a live view of the scene being authored, with the panels
//! that edit it.
//!
//! Layers depend downwards only: `panels` -> `viewport` -> `preview` ->
//! `fr_authoring` commands -> document. Engine subsystems plug in through
//! `subsystems`, one folder each, registered in one place.
//!
//! Modules:
//!
//! - `app`: the application state, messages, the loop, menus and layout
//! - `panels`: Hierarchy, Project, Inspector, Console and their registry
//! - `viewport`: the editor camera, picking and the transform gizmo
//! - `preview`: the document realized into a live stage, never simulated
//! - `subsystems`: one folder per engine subsystem being authored
//! - `run`: building and launching the game as a separate process
//! - `options`: the command line
//! - `settings`: per-user settings, per-project state and project discovery

pub mod app;
pub mod options;
pub mod panels;
pub mod preview;
pub mod run;
pub mod settings;
pub mod subsystems;
pub mod viewport;
