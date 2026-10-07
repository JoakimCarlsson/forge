//! Asset import errors.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use fr_document::{DocumentError, Guid};

/// A failure to read or interpret an asset file.
#[derive(Debug)]
pub enum AssetError {
    /// The file could not be read or parsed.
    Import {
        /// The file that was being imported.
        path: PathBuf,
        /// What the importer reported.
        source: Box<dyn Error + Send + Sync>,
    },
    /// The file parsed but describes something this crate cannot represent.
    Malformed {
        /// The file that was being imported.
        path: PathBuf,
        /// What is wrong with it.
        reason: String,
    },
    /// A scene, prefab or sidecar could not be read or understood.
    Document(DocumentError),
    /// No asset of the library has the identity.
    Missing {
        /// The identity that was asked for.
        id: Guid,
    },
    /// The asset exists but is not of the kind that was asked for.
    WrongKind {
        /// The path of the asset, relative to the asset root.
        path: String,
        /// The kind that was asked for.
        expected: &'static str,
    },
}

impl AssetError {
    /// An import failure of `path` caused by `source`.
    pub(crate) fn import(
        path: &std::path::Path,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Import {
            path: path.to_path_buf(),
            source: Box::new(source),
        }
    }

    /// A malformed-content failure of `path` described by `reason`.
    pub(crate) fn malformed(path: &std::path::Path, reason: impl Into<String>) -> Self {
        Self::Malformed {
            path: path.to_path_buf(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for AssetError {
    /// Writes the file and the cause of the failure.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Import { path, source } => {
                write!(f, "cannot import {}: {source}", path.display())
            }
            Self::Malformed { path, reason } => {
                write!(f, "cannot use {}: {reason}", path.display())
            }
            Self::Document(error) => write!(f, "{error}"),
            Self::Missing { id } => write!(f, "no asset has the identity {}", id.to_text()),
            Self::WrongKind { path, expected } => write!(f, "{path} is not a {expected}"),
        }
    }
}

impl Error for AssetError {
    /// The importer's own error, when there is one.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Import { source, .. } => Some(source.as_ref()),
            Self::Document(error) => Some(error),
            Self::Malformed { .. } | Self::Missing { .. } | Self::WrongKind { .. } => None,
        }
    }
}

impl From<DocumentError> for AssetError {
    /// Wraps a failure of the authored data.
    fn from(error: DocumentError) -> Self {
        Self::Document(error)
    }
}
