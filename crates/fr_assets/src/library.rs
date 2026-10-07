//! The typed asset library over a project's asset root: models, prefabs and
//! scenes loaded by identity or path and cached.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fr_document::{
    AssetIndex, AssetKind, AssetReference, Guid, PrefabAsset, SceneAsset, load_asset_meta,
    read_prefab_text, read_scene_text, read_text_file, relative_text,
};

use crate::error::AssetError;
use crate::gltf_import::load_gltf;
use crate::model::ModelData;

/// The assets of one asset root, found by their sidecars and loaded on demand.
///
/// A loaded asset is cached under its identity and shared, so loading it again
/// is free until [`AssetLibrary::invalidate`] drops it. The library is the
/// engine's only way to read project files at runtime; no filesystem root
/// reaches the scene.
pub struct AssetLibrary {
    /// The directory every asset path is relative to.
    root: PathBuf,
    /// The identity and path of every asset.
    index: AssetIndex,
    /// The models loaded so far.
    models: HashMap<Guid, Arc<ModelData>>,
    /// The prefabs loaded so far.
    prefabs: HashMap<Guid, Arc<PrefabAsset>>,
    /// The scenes loaded so far.
    scenes: HashMap<Guid, Arc<SceneAsset>>,
    /// How often each asset was invalidated, which a hot reload compares.
    revisions: HashMap<Guid, u64>,
}

impl AssetLibrary {
    /// Opens the library of an asset root, scanning its sidecars. Sidecars that
    /// cannot be read are reported by [`AssetLibrary::index`].
    pub fn open(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            index: AssetIndex::build(root),
            models: HashMap::new(),
            prefabs: HashMap::new(),
            scenes: HashMap::new(),
            revisions: HashMap::new(),
        }
    }

    /// The directory every asset path is relative to.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The identity and path of every asset.
    pub fn index(&self) -> &AssetIndex {
        &self.index
    }

    /// Scans the sidecars again, keeping what is loaded.
    pub fn rescan(&mut self) {
        self.index = AssetIndex::build(&self.root);
    }

    /// The absolute path of the asset with an identity.
    pub fn path_of(&self, id: Guid) -> Option<PathBuf> {
        self.index.absolute_path(id)
    }

    /// The identity of the asset at a path relative to the root.
    pub fn guid_of(&self, path: &str) -> Option<Guid> {
        self.index.find_path(path).map(|asset| asset.id)
    }

    /// A reference to the asset at a path relative to the root.
    pub fn reference(&self, path: &str) -> Option<AssetReference> {
        self.index.find_path(path).map(|asset| AssetReference {
            asset: asset.id,
            last_known_path: asset.path.clone(),
        })
    }

    /// A path as the library names it: relative to the root, with forward
    /// slashes. A path inside the root is made relative; any other is kept.
    pub fn relative_path(&self, path: &Path) -> String {
        if path.is_absolute() {
            relative_text(&self.root, path)
        } else {
            relative_text(Path::new(""), path)
        }
    }

    /// The path of an asset of a kind, for a load.
    fn locate(&self, id: Guid, kind: AssetKind) -> Result<PathBuf, AssetError> {
        let asset = self.index.find(id).ok_or(AssetError::Missing { id })?;
        if AssetKind::of_path(&asset.path) != kind {
            return Err(AssetError::WrongKind {
                path: asset.path.clone(),
                expected: kind.name(),
            });
        }
        Ok(self.root.join(&asset.path))
    }

    /// The model with an identity, loading it the first time.
    ///
    /// # Errors
    ///
    /// When no model has the identity or the file cannot be imported.
    pub fn model(&mut self, id: Guid) -> Result<Arc<ModelData>, AssetError> {
        if let Some(model) = self.models.get(&id) {
            return Ok(Arc::clone(model));
        }
        let path = self.locate(id, AssetKind::Model)?;
        let model = Arc::new(load_gltf(&path)?);
        self.models.insert(id, Arc::clone(&model));
        Ok(model)
    }

    /// The prefab with an identity, loading it the first time.
    ///
    /// # Errors
    ///
    /// When no prefab has the identity or the file cannot be read, or is of
    /// another format version.
    pub fn prefab(&mut self, id: Guid) -> Result<Arc<PrefabAsset>, AssetError> {
        if let Some(prefab) = self.prefabs.get(&id) {
            return Ok(Arc::clone(prefab));
        }
        let path = self.locate(id, AssetKind::Prefab)?;
        let text = read_text_file(&path)?;
        let mut asset = read_prefab_text(&text).map_err(|error| error.in_file(&path))?;
        asset.id = id;
        let prefab = Arc::new(asset);
        self.prefabs.insert(id, Arc::clone(&prefab));
        Ok(prefab)
    }

    /// The scene with an identity, loading it the first time.
    ///
    /// # Errors
    ///
    /// When no scene has the identity or the file cannot be read, or is of
    /// another format version.
    pub fn scene(&mut self, id: Guid) -> Result<Arc<SceneAsset>, AssetError> {
        if let Some(scene) = self.scenes.get(&id) {
            return Ok(Arc::clone(scene));
        }
        let path = self.locate(id, AssetKind::Scene)?;
        let scene = Arc::new(Self::read_scene(&path)?);
        self.scenes.insert(id, Arc::clone(&scene));
        Ok(scene)
    }

    /// Reads a scene file, taking its identity from its sidecar when it has one.
    fn read_scene(path: &Path) -> Result<SceneAsset, AssetError> {
        let text = read_text_file(path)?;
        let mut asset = read_scene_text(&text).map_err(|error| error.in_file(path))?;
        if let Some(meta) = load_asset_meta(path)? {
            asset.id = meta.id;
        }
        Ok(asset)
    }

    /// The scene at a path, relative to the root or absolute. A scene with a
    /// sidecar is cached under its identity; one without is read each time.
    ///
    /// # Errors
    ///
    /// When the file cannot be read or is not a scene of the current version.
    pub fn scene_at(&mut self, path: &Path) -> Result<Arc<SceneAsset>, AssetError> {
        let relative = self.relative_path(path);
        if let Some(id) = self.guid_of(&relative) {
            return self.scene(id);
        }
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        Ok(Arc::new(Self::read_scene(&absolute)?))
    }

    /// Drops the cached asset with an identity, so the next load reads it again,
    /// and counts the change in [`AssetLibrary::revision`].
    pub fn invalidate(&mut self, id: Guid) {
        self.models.remove(&id);
        self.prefabs.remove(&id);
        self.scenes.remove(&id);
        *self.revisions.entry(id).or_insert(0) += 1;
    }

    /// How many times the asset with an identity was invalidated.
    pub fn revision(&self, id: Guid) -> u64 {
        self.revisions.get(&id).copied().unwrap_or(0)
    }
}
