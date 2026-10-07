//! The kinds of asset a project holds, told by file extension.

/// What a file of a project is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    /// A `.scene` document.
    Scene,
    /// A `.prefab` document.
    Prefab,
    /// A glTF or GLB model.
    Model,
    /// Any other file.
    Other,
}

impl AssetKind {
    /// The kind of the file at a path, by its extension.
    pub fn of_path(path: &str) -> Self {
        match path.rsplit_once('.').map(|(_, extension)| extension) {
            Some("scene") => Self::Scene,
            Some("prefab") => Self::Prefab,
            Some("gltf" | "glb") => Self::Model,
            _ => Self::Other,
        }
    }

    /// The name of the kind, for messages.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Prefab => "prefab",
            Self::Model => "model",
            Self::Other => "asset",
        }
    }
}
