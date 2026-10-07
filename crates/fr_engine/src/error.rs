//! The failures a run can end with.

use std::error::Error;
use std::fmt;

use fr_assets::AssetError;
use fr_document::DocumentError;
use fr_project::ProjectError;
use fr_render::RenderError;
use fr_scene::SceneError;
use fr_window::WindowError;

/// Why a callback, a scene load or the run itself failed: a message that names
/// the cause. A failing callback ends the frame loop and is reported as the
/// reason the run failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineError {
    /// What went wrong.
    message: String,
}

/// The result of an engine operation.
pub type EngineResult<T = ()> = Result<T, EngineError>;

impl EngineError {
    /// An error with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// What went wrong.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for EngineError {
    /// Writes the message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for EngineError {}

impl From<String> for EngineError {
    /// An error from a message.
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for EngineError {
    /// An error from a message.
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// Implements `From` for the engine error from error types that only need
/// their display text.
macro_rules! from_display {
    ($($source:ty),* $(,)?) => {
        $(
            impl From<$source> for EngineError {
                /// An error with the display text of the cause.
                fn from(error: $source) -> Self {
                    Self::new(error.to_string())
                }
            }
        )*
    };
}

from_display!(
    AssetError,
    DocumentError,
    ProjectError,
    RenderError,
    SceneError,
    WindowError,
);
