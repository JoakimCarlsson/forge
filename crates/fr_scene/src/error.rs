//! The failures of building and running a scene.

use std::error::Error;
use std::fmt;

use fr_assets::AssetError;
use fr_document::DocumentError;

/// Why a scene operation failed.
#[derive(Debug)]
pub enum SceneError {
    /// An authored file could not be read or understood.
    Document(DocumentError),
    /// An asset could not be loaded.
    Asset(AssetError),
    /// A document breaks the rules of its component types.
    Invalid {
        /// The errors the validation found, one per line.
        problems: Vec<String>,
    },
    /// A component could not be realized.
    Realization {
        /// The component type and what is wrong with it.
        message: String,
    },
    /// An entity handle does not name a live entity.
    MissingEntity,
    /// The graphics side could not provide a resource the frame needs.
    Resource {
        /// What failed.
        message: String,
    },
}

impl SceneError {
    /// A realization failure with a message.
    pub fn realization(message: impl Into<String>) -> Self {
        Self::Realization {
            message: message.into(),
        }
    }

    /// A resource failure with a message.
    pub fn resource(message: impl Into<String>) -> Self {
        Self::Resource {
            message: message.into(),
        }
    }
}

impl fmt::Display for SceneError {
    /// Writes the cause; a validation failure lists every problem.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => write!(f, "{error}"),
            Self::Asset(error) => write!(f, "{error}"),
            Self::Invalid { problems } => {
                write!(f, "the document is invalid: {}", problems.join("; "))
            }
            Self::Realization { message } => write!(f, "{message}"),
            Self::MissingEntity => f.write_str("the entity no longer exists"),
            Self::Resource { message } => write!(f, "{message}"),
        }
    }
}

impl Error for SceneError {
    /// The wrapped failure, when there is one.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Asset(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DocumentError> for SceneError {
    /// Wraps a failure of the authored data.
    fn from(error: DocumentError) -> Self {
        Self::Document(error)
    }
}

impl From<AssetError> for SceneError {
    /// Wraps a failure to load an asset.
    fn from(error: AssetError) -> Self {
        Self::Asset(error)
    }
}
