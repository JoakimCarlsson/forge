//! The failures of opening, scaffolding and building a project.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use fr_document::DocumentError;

/// Why a project operation failed.
#[derive(Debug)]
pub enum ProjectError {
    /// The directory holds no `project.forge`.
    NotAProject {
        /// The directory that was opened.
        root: PathBuf,
    },
    /// A `project.forge` line could not be understood.
    Settings {
        /// The file concerned.
        path: PathBuf,
        /// What is wrong with it.
        reason: String,
    },
    /// The directory already holds a project.
    AlreadyExists {
        /// The existing project's directory.
        root: PathBuf,
    },
    /// A file or directory could not be written, or an authored file failed.
    Document(DocumentError),
    /// A build step did not run or did not succeed.
    Build {
        /// What the step was doing.
        description: String,
        /// Why it failed.
        reason: String,
    },
}

impl fmt::Display for ProjectError {
    /// Writes the cause with the path it concerns.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAProject { root } => {
                write!(f, "{} has no project.forge", root.display())
            }
            Self::Settings { path, reason } => write!(f, "{}: {reason}", path.display()),
            Self::AlreadyExists { root } => {
                write!(f, "{} already holds a project", root.display())
            }
            Self::Document(error) => write!(f, "{error}"),
            Self::Build {
                description,
                reason,
            } => write!(f, "{description} failed: {reason}"),
        }
    }
}

impl Error for ProjectError {
    /// The authored-data failure, when there is one.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DocumentError> for ProjectError {
    /// Wraps a failure of the authored data.
    fn from(error: DocumentError) -> Self {
        Self::Document(error)
    }
}
