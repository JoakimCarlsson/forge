//! The index from asset identity to path under an asset root, rebuilt by
//! scanning the sidecars.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::files::{AssetScan, DuplicateAsset, IndexedAsset, scan_assets};
use crate::guid::Guid;

/// Every asset of an asset root, found by its sidecar. The index is
/// disposable: it is rebuilt by scanning, never saved.
#[derive(Clone, Debug, Default)]
pub struct AssetIndex {
    /// The directory the paths are relative to.
    root: PathBuf,
    /// The assets in path order.
    assets: Vec<IndexedAsset>,
    /// The position in `assets` of each identity.
    by_id: HashMap<Guid, usize>,
    /// The position in `assets` of each path.
    by_path: HashMap<String, usize>,
    /// The assets whose identity another asset holds.
    duplicates: Vec<DuplicateAsset>,
    /// The sidecars that could not be read.
    problems: Vec<String>,
}

impl AssetIndex {
    /// Scans the sidecars under a root.
    pub fn build(root: &Path) -> Self {
        let AssetScan {
            assets,
            duplicates,
            problems,
        } = scan_assets(root);
        let by_id = assets
            .iter()
            .enumerate()
            .map(|(position, asset)| (asset.id, position))
            .collect();
        let by_path = assets
            .iter()
            .enumerate()
            .map(|(position, asset)| (asset.path.clone(), position))
            .collect();
        Self {
            root: root.to_path_buf(),
            assets,
            by_id,
            by_path,
            duplicates,
            problems,
        }
    }

    /// The directory the asset paths are relative to.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every asset in path order.
    pub fn assets(&self) -> &[IndexedAsset] {
        &self.assets
    }

    /// The asset with an identity.
    pub fn find(&self, id: Guid) -> Option<&IndexedAsset> {
        self.by_id
            .get(&id)
            .and_then(|index| self.assets.get(*index))
    }

    /// The asset at a path relative to the root, written with forward slashes.
    pub fn find_path(&self, path: &str) -> Option<&IndexedAsset> {
        self.by_path
            .get(path)
            .and_then(|index| self.assets.get(*index))
    }

    /// The absolute path of the asset with an identity.
    pub fn absolute_path(&self, id: Guid) -> Option<PathBuf> {
        self.find(id).map(|asset| self.root.join(&asset.path))
    }

    /// The assets that share an identity with an earlier one.
    pub fn duplicates(&self) -> &[DuplicateAsset] {
        &self.duplicates
    }

    /// The sidecars that could not be read, each with the reason.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }
}
