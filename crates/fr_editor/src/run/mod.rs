//! Building and launching a project's game as a separate process.
//!
//! No game code is ever loaded into the editor: the game is built by running
//! the project's `fr_project::build_steps` and started from the
//! `fr_authoring::LaunchPlan`, and the editor only sees the lines the child
//! prints and how it ends. Everything here is headless; the Console panel
//! shows the [`RunEvent`]s.
//!
//! Modules:
//!
//! - `output`: the lines and events a run produces
//! - `pipe`: spawning a child with its output drained by reader threads
//! - `process`: [`GameProcess`], one running child
//! - `build_job`: [`BuildJob`], the build steps on a worker thread
//! - `runner`: [`Runner`], the Idle, Building and Running state machine

mod build_job;
mod output;
mod pipe;
mod process;
mod runner;

pub use build_job::BuildJob;
pub use output::{MESSAGE_PREFIX, OutputLine, RunEvent, Stream, notice};
pub use process::GameProcess;
pub use runner::{Planner, RunState, Runner};
