//! Realizing a document into a fresh stage.

use fr_assets::AssetLibrary;
use fr_authoring::{Document, DocumentKind};
use fr_document::{Guid, SceneAsset, SceneSettings};
use fr_scene::{RealizedEntity, Stage};

/// A stage made from a document and what the walk reported.
pub struct Built {
    /// The stage, never stepped.
    pub stage: Stage,
    /// The entities the walk created, with the prefab instances above each.
    pub entities: Vec<RealizedEntity>,
    /// Messages about data that is kept but inert.
    pub diagnostics: Vec<String>,
}

/// Realizes a document through the stage's registered adapters. A scene is
/// realized as it is; a prefab is realized as a scene of its own content with
/// the default settings, so the nodes it holds are the nodes of the document.
///
/// # Errors
///
/// The message of the failure: the validation errors of the document, or the
/// asset or component that could not be realized.
pub fn build_stage(assets: &mut AssetLibrary, document: &Document) -> Result<Built, String> {
    let settings = match document.kind() {
        DocumentKind::Scene => *document.settings(),
        DocumentKind::Prefab => SceneSettings::default(),
    };
    let asset = SceneAsset {
        id: Guid::NONE,
        content: document.content().clone(),
        settings,
    };
    let mut stage = Stage::new();
    let realization = stage
        .realize_scene(assets, document.schemas(), &asset)
        .map_err(|error| error.to_string())?;
    Ok(Built {
        stage,
        entities: realization.entities,
        diagnostics: realization.diagnostics,
    })
}
