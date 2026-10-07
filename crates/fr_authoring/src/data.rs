//! The editable content of a document and the snapshots history keeps of it.

use fr_document::{EntityDocument, SceneSettings};

/// Everything a command can change in a document: the entity records and the
/// scene settings. A prefab keeps default settings that are never written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentData {
    /// The entities and prefab instances.
    pub content: EntityDocument,
    /// The scene settings.
    pub settings: SceneSettings,
}

/// The content of a document at one state, as history stores it.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The content.
    pub data: DocumentData,
    /// The identity of the state, which dirty tracking compares.
    pub state: u64,
}
