//! Projects of the forge engine: the `project.forge` file, the listing of a
//! project's assets with their identities, scaffolding of a new game from the
//! templates in `crates/fr_project/templates` and the cargo steps that build it.
//!
//! A project is a directory with a `project.forge` beside an asset root of
//! models, prefabs and scenes and, usually, the game's own Cargo project. The
//! editor of a later stage and the game's own `main` both open it through
//! [`Project`]; nothing here knows a window or an engine object.
//!
//! The crate root is the facade and nothing else.

mod build;
mod error;
mod listing;
mod project;
mod scaffold;
mod settings;

pub use build::{BuildStep, build_steps};
pub use error::ProjectError;
pub use listing::{ProjectEntry, list_project};
pub use project::Project;
pub use scaffold::{create_project, engine_crate_dir, package_name};
pub use settings::{ProjectSettings, SETTINGS_FILE, read_project_text, write_project_text};
