//! The failures of authoring operations.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use fr_document::{DocumentError, Guid, PropertyType, Severity};
use fr_project::ProjectError;

use crate::workspace::DocumentId;

/// Why an authoring operation failed. A failed operation leaves the document,
/// history and selection as they were.
#[derive(Debug)]
pub enum AuthoringError {
    /// An authored file could not be read, written or understood.
    Document(DocumentError),
    /// A project operation failed.
    Project(ProjectError),
    /// The document has validation errors and was not saved.
    Invalid {
        /// The error diagnostics, one line each.
        messages: Vec<String>,
    },
    /// A node of the hierarchy does not exist.
    NodeNotFound {
        /// The identity that was looked up.
        id: Guid,
    },
    /// A node exists but is not the kind the operation needs.
    WrongNodeKind {
        /// The identity of the node.
        id: Guid,
        /// What the operation needs, such as "an entity".
        expected: &'static str,
    },
    /// An entity has no component with the identity.
    ComponentNotFound {
        /// The entity that was searched.
        entity: Guid,
        /// The component identity that was looked up.
        component: Guid,
    },
    /// A reparent would make a node its own ancestor.
    ReparentCycle {
        /// The node being moved.
        node: Guid,
        /// The requested parent, which is the node or lies under it.
        parent: Guid,
    },
    /// A component type is not registered.
    UnknownComponentType {
        /// The type key.
        kind: String,
    },
    /// A component cannot be added or removed.
    ComponentRejected {
        /// The type key.
        kind: String,
        /// Why it cannot.
        reason: String,
    },
    /// A property is neither in the component's schema nor in its record.
    UnknownProperty {
        /// The component type key.
        kind: String,
        /// The property key.
        key: String,
    },
    /// A value has another type than the property.
    PropertyTypeMismatch {
        /// The property key.
        key: String,
        /// The type the property has.
        expected: PropertyType,
        /// The type the value has.
        found: PropertyType,
    },
    /// A value is not acceptable for a property.
    InvalidValue {
        /// The property key, or the field name.
        key: String,
        /// Why the value is not acceptable.
        reason: String,
    },
    /// A name is empty or cannot be used as a file or folder name.
    InvalidName {
        /// The name as given.
        name: String,
        /// Why it cannot be used.
        reason: String,
    },
    /// A file or folder to create already exists.
    AlreadyExists {
        /// The existing path.
        path: PathBuf,
    },
    /// A path is not a `.scene` or `.prefab` file.
    UnsupportedFile {
        /// The path concerned.
        path: PathBuf,
    },
    /// A path lies outside the asset root.
    OutsideAssetRoot {
        /// The path concerned.
        path: PathBuf,
        /// The asset root.
        asset_root: PathBuf,
    },
    /// The workspace has no document with the handle.
    UnknownDocument {
        /// The handle that was looked up.
        id: DocumentId,
    },
    /// A gesture operation was used while no gesture is active.
    NoGesture,
    /// An operation that cannot run during a gesture was used while one is active.
    GestureActive,
}

impl AuthoringError {
    /// A rejected component operation on a type.
    pub fn rejected(kind: &str, reason: impl Into<String>) -> Self {
        Self::ComponentRejected {
            kind: kind.to_owned(),
            reason: reason.into(),
        }
    }

    /// An unacceptable value for a property or field.
    pub fn invalid_value(key: &str, reason: impl Into<String>) -> Self {
        Self::InvalidValue {
            key: key.to_owned(),
            reason: reason.into(),
        }
    }

    /// The error of a document with validation errors, listing the error
    /// diagnostics among `diagnostics`.
    pub fn invalid(diagnostics: &[fr_document::Diagnostic]) -> Self {
        Self::Invalid {
            messages: diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Error)
                .map(|diagnostic| diagnostic.message.clone())
                .collect(),
        }
    }
}

impl fmt::Display for AuthoringError {
    /// Writes the cause in a sentence an editor can show as it is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => write!(f, "{error}"),
            Self::Project(error) => write!(f, "{error}"),
            Self::Invalid { messages } => {
                write!(f, "the document has errors: {}", messages.join("; "))
            }
            Self::NodeNotFound { id } => write!(f, "no node {}", id.to_text()),
            Self::WrongNodeKind { id, expected } => {
                write!(f, "node {} is not {expected}", id.to_text())
            }
            Self::ComponentNotFound { entity, component } => write!(
                f,
                "entity {} has no component {}",
                entity.to_text(),
                component.to_text()
            ),
            Self::ReparentCycle { node, parent } => write!(
                f,
                "cannot move {} under {}: that would make it its own ancestor",
                node.to_text(),
                parent.to_text()
            ),
            Self::UnknownComponentType { kind } => {
                write!(f, "component type {kind} is not registered")
            }
            Self::ComponentRejected { kind, reason } => write!(f, "{kind}: {reason}"),
            Self::UnknownProperty { kind, key } => {
                write!(f, "{kind} has no property {key}")
            }
            Self::PropertyTypeMismatch {
                key,
                expected,
                found,
            } => write!(
                f,
                "property {key} is {} but the value is {}",
                expected.key(),
                found.key()
            ),
            Self::InvalidValue { key, reason } => write!(f, "{key}: {reason}"),
            Self::InvalidName { name, reason } => write!(f, "invalid name \"{name}\": {reason}"),
            Self::AlreadyExists { path } => write!(f, "{} already exists", path.display()),
            Self::UnsupportedFile { path } => {
                write!(f, "{} is not a .scene or .prefab file", path.display())
            }
            Self::OutsideAssetRoot { path, asset_root } => write!(
                f,
                "{} is outside the asset root {}",
                path.display(),
                asset_root.display()
            ),
            Self::UnknownDocument { id } => write!(f, "no open document {}", id.value()),
            Self::NoGesture => write!(f, "no gesture is active"),
            Self::GestureActive => write!(f, "a gesture is active"),
        }
    }
}

impl Error for AuthoringError {
    /// The failure of the authored data or the project, when there is one.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Project(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DocumentError> for AuthoringError {
    /// Wraps a failure of the authored data.
    fn from(error: DocumentError) -> Self {
        Self::Document(error)
    }
}

impl From<ProjectError> for AuthoringError {
    /// Wraps a failure of a project operation.
    fn from(error: ProjectError) -> Self {
        Self::Project(error)
    }
}
