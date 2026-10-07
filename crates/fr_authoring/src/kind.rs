//! The two kinds of document the authoring layer opens.

use std::path::Path;

use fr_document::FormatKind;

/// Whether a document is a scene or a prefab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    /// A `.scene` file: entities, prefab instances and scene settings.
    Scene,
    /// A `.prefab` file: one reusable hierarchy.
    Prefab,
}

impl DocumentKind {
    /// The kind a file name's extension selects; none for other files.
    pub fn of_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("scene") => Some(Self::Scene),
            Some("prefab") => Some(Self::Prefab),
            _ => None,
        }
    }

    /// The file extension without the dot.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Prefab => "prefab",
        }
    }

    /// The lower case name of the kind, for display.
    pub const fn name(self) -> &'static str {
        self.extension()
    }

    /// The file format the kind is stored as.
    pub const fn format(self) -> FormatKind {
        match self {
            Self::Scene => FormatKind::Scene,
            Self::Prefab => FormatKind::Prefab,
        }
    }
}
