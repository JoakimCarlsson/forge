//! What the editor remembers: per-user settings in the config directory, the
//! per-project state beside the project, and the projects found for the picker.
//!
//! Nothing here writes in a project's source tree except
//! [`ProjectState`], whose folder keeps its own `.gitignore`.
//!
//! Modules:
//!
//! - `ini`: the `key = value` text format both files use
//! - `user`: [`Settings`], the per-user editor settings
//! - `project_state`: [`ProjectState`], the open documents of one project
//! - `discovery`: [`discover_projects`], the projects under a folder

mod discovery;
mod ini;
mod project_state;
mod user;

pub use discovery::{DiscoveredProject, discover_projects};
pub use project_state::{PROJECT_STATE_DIRECTORY, ProjectState};
pub use user::{MAX_RECENT_PROJECTS, Settings};
