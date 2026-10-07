//! The ordered set of selected nodes of a document.

use fr_document::{EntityDocument, Guid};

/// The selected nodes in the order they were selected, with one primary node
/// that inspectors show. Nodes that stop existing are pruned with
/// [`Selection::prune`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// The selected nodes, oldest first.
    nodes: Vec<Guid>,
    /// The primary node, always one of `nodes`.
    primary: Option<Guid>,
}

impl Selection {
    /// An empty selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Selects exactly one node, which becomes primary.
    pub fn set(&mut self, node: Guid) {
        self.nodes = vec![node];
        self.primary = Some(node);
    }

    /// Selects exactly these nodes in order; the last becomes primary.
    pub fn set_all(&mut self, nodes: impl IntoIterator<Item = Guid>) {
        self.nodes.clear();
        for node in nodes {
            if !self.nodes.contains(&node) {
                self.nodes.push(node);
            }
        }
        self.primary = self.nodes.last().copied();
    }

    /// Adds a node and makes it primary, or removes it when it is selected;
    /// removing the primary node promotes the last remaining one.
    pub fn toggle(&mut self, node: Guid) {
        if self.contains(node) {
            self.remove(node);
        } else {
            self.extend(node);
        }
    }

    /// Adds a node, if it is not selected already, and makes it primary.
    pub fn extend(&mut self, node: Guid) {
        if !self.nodes.contains(&node) {
            self.nodes.push(node);
        }
        self.primary = Some(node);
    }

    /// Adds every node between the primary node and a target in the document's
    /// hierarchy order, as a shift click does, and makes the target primary.
    /// Without a primary node the target alone is selected.
    pub fn extend_to(&mut self, document: &EntityDocument, target: Guid) {
        let Some(anchor) = self.primary else {
            self.set(target);
            return;
        };
        let order: Vec<Guid> = document
            .ordered_nodes()
            .into_iter()
            .map(|node| node.id)
            .collect();
        let from = order.iter().position(|id| *id == anchor);
        let to = order.iter().position(|id| *id == target);
        let (Some(from), Some(to)) = (from, to) else {
            self.extend(target);
            return;
        };
        let (low, high) = if from <= to { (from, to) } else { (to, from) };
        for id in &order[low..=high] {
            if !self.nodes.contains(id) {
                self.nodes.push(*id);
            }
        }
        self.primary = Some(target);
    }

    /// Removes a node from the selection.
    pub fn remove(&mut self, node: Guid) {
        self.nodes.retain(|selected| *selected != node);
        if self.primary == Some(node) {
            self.primary = self.nodes.last().copied();
        }
    }

    /// Empties the selection.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.primary = None;
    }

    /// Drops the nodes the document no longer has; returns whether any went.
    pub fn prune(&mut self, document: &EntityDocument) -> bool {
        let before = self.nodes.len();
        self.nodes.retain(|node| document.has_node(*node));
        if self
            .primary
            .is_some_and(|primary| !self.nodes.contains(&primary))
        {
            self.primary = self.nodes.last().copied();
        }
        self.nodes.len() != before
    }

    /// The selected nodes, oldest first.
    pub fn nodes(&self) -> &[Guid] {
        &self.nodes
    }

    /// The primary node.
    pub fn primary(&self) -> Option<Guid> {
        self.primary
    }

    /// Whether a node is selected.
    pub fn contains(&self, node: Guid) -> bool {
        self.nodes.contains(&node)
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The number of selected nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
}
