//! What the Hierarchy remembers between frames: scroll, expansion per
//! document, the filter, the node being renamed and the row being carried.

use std::collections::{HashMap, HashSet};

use fr_authoring::DocumentId;
use fr_document::Guid;
use fr_ui::{DropHint, Scroll};

/// The row a carried row would land on and where on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropTarget {
    /// The row under the pointer; unset for the document's own row.
    pub node: Guid,
    /// The part of the row the pointer is over.
    pub hint: DropHint,
}

/// A row being carried across the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HierarchyDrag {
    /// The node being carried.
    pub node: Guid,
    /// Where it would land, when it may land where the pointer is.
    pub target: Option<DropTarget>,
}

/// The Hierarchy panel's state.
#[derive(Debug, Default)]
pub struct HierarchyState {
    /// How far the tree is scrolled.
    pub scroll: Scroll,
    /// The collapsed nodes of each open document; the document's own row is
    /// the unset identity. Everything else starts expanded.
    pub collapsed: HashMap<DocumentId, HashSet<Guid>>,
    /// The text rows must contain to be listed.
    pub filter: String,
    /// The node whose name is being typed over.
    pub renaming: Option<Guid>,
    /// The row being carried.
    pub drag: Option<HierarchyDrag>,
}

impl HierarchyState {
    /// The collapsed nodes of a document.
    pub fn collapsed_in(&self, document: DocumentId) -> Option<&HashSet<Guid>> {
        self.collapsed.get(&document)
    }

    /// Expands a collapsed node and collapses an expanded one.
    pub fn toggle(&mut self, document: DocumentId, node: Guid) {
        let set = self.collapsed.entry(document).or_default();
        if !set.remove(&node) {
            set.insert(node);
        }
    }

    /// Expands a node.
    pub fn expand(&mut self, document: DocumentId, node: Guid) {
        if let Some(set) = self.collapsed.get_mut(&document) {
            set.remove(&node);
        }
    }

    /// Forgets what is kept for a document that was closed.
    pub fn forget(&mut self, document: DocumentId) {
        self.collapsed.remove(&document);
    }
}
