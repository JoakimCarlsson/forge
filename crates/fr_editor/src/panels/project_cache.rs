//! The cached listing of a project's asset root.
//!
//! Listing walks the disk and scans every sidecar, so the Project panel and the
//! Inspector's asset pickers share one listing that is built on first use and
//! kept until something invalidates it: Refresh, creating a file or folder, or
//! another project.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use fr_document::AssetIndex;
use fr_project::{Project, ProjectEntry, list_project};

/// What a project's asset root holds.
#[derive(Debug, Default)]
pub struct ProjectListing {
    /// Every folder and file in tree order.
    pub entries: Vec<ProjectEntry>,
    /// The identity and path of every asset.
    pub index: AssetIndex,
}

/// A listing built on first use and kept until invalidated.
#[derive(Debug, Default)]
pub struct ProjectCache {
    /// The asset root listed and the listing.
    cell: RefCell<Option<(PathBuf, Rc<ProjectListing>)>>,
}

impl ProjectCache {
    /// The listing of a project, built now when there is none for its asset
    /// root.
    pub fn get(&self, project: &Project) -> Rc<ProjectListing> {
        let root = project.asset_root();
        let mut cell = self.cell.borrow_mut();
        if let Some((listed, listing)) = cell.as_ref()
            && *listed == root
        {
            return listing.clone();
        }
        let listing = Rc::new(ProjectListing {
            entries: list_project(project),
            index: project.asset_index(),
        });
        *cell = Some((root, listing.clone()));
        listing
    }

    /// Drops the listing so the next [`ProjectCache::get`] reads the disk again.
    pub fn invalidate(&self) {
        *self.cell.borrow_mut() = None;
    }
}
