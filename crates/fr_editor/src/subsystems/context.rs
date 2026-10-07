//! What a contribution may consult to compute the local bounds of a component.

use std::collections::HashMap;

use fr_assets::AssetLibrary;
use fr_document::Guid;
use fr_math::Aabb;

/// The assets a bounds computation reads from, with the bounds of each model
/// remembered for the length of one pass so a model used by many nodes is
/// measured once.
pub struct BoundsContext<'a> {
    /// The library models are read from.
    assets: &'a mut AssetLibrary,
    /// The bounds measured so far, by model identity.
    models: HashMap<Guid, Option<Aabb>>,
}

impl<'a> BoundsContext<'a> {
    /// A context over an asset library.
    pub fn new(assets: &'a mut AssetLibrary) -> Self {
        Self {
            assets,
            models: HashMap::new(),
        }
    }

    /// The bounds of a model in its own space; none when the model cannot be
    /// read or has no geometry.
    pub fn model_bounds(&mut self, model: Guid) -> Option<Aabb> {
        if let Some(known) = self.models.get(&model) {
            return *known;
        }
        let bounds = self.assets.model(model).ok().and_then(|data| data.bounds());
        self.models.insert(model, bounds);
        bounds
    }
}
