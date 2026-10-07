//! The broad phase: one dynamic AABB tree per body type and the buffer of proxies that moved
//! since the last pair update.

use fr_math::Vec3;

use crate::body::BodyType;
use crate::shape::ShapeId;
use crate::tree::{NodeId, Tree};
use fr_math::Aabb;

/// The trees of the three body types and the moved proxies.
#[derive(Clone, Debug, Default)]
pub(crate) struct BroadPhase {
    /// The trees indexed by [`BroadPhase::tree_index`].
    trees: [Tree; 3],
    /// The shapes whose bounds changed since the last pair update, in order.
    pub(crate) move_buffer: Vec<ShapeId>,
}

impl BroadPhase {
    /// The index of the tree that holds shapes of bodies of type `body_type`.
    pub(crate) fn tree_index(body_type: BodyType) -> usize {
        match body_type {
            BodyType::Static => 0,
            BodyType::Kinematic => 1,
            BodyType::Dynamic => 2,
        }
    }

    /// Adds a proxy for shape slot `shape_index` and returns its leaf.
    pub(crate) fn create_proxy(
        &mut self,
        body_type: BodyType,
        aabb: &Aabb,
        shape_index: u32,
    ) -> NodeId {
        self.trees[Self::tree_index(body_type)].create_proxy(aabb, shape_index)
    }

    /// Removes a proxy.
    pub(crate) fn destroy_proxy(&mut self, body_type: BodyType, node: NodeId) {
        self.trees[Self::tree_index(body_type)].destroy_proxy(node);
    }

    /// Moves a proxy to new bounds.
    pub(crate) fn move_proxy(&mut self, body_type: BodyType, node: NodeId, aabb: &Aabb) {
        self.trees[Self::tree_index(body_type)].move_proxy(node, aabb);
    }

    /// Calls `callback` with the shape slot of every proxy of the tree of `body_type` that
    /// overlaps `aabb`, until it returns false. Returns whether the walk completed.
    pub(crate) fn query_tree<F>(&self, body_type: BodyType, aabb: &Aabb, mut callback: F) -> bool
    where
        F: FnMut(u32) -> bool,
    {
        let tree = &self.trees[Self::tree_index(body_type)];
        let mut completed = true;
        tree.query(aabb, |node| {
            completed = callback(tree.user_data(node));
            completed
        });
        completed
    }

    /// Calls `callback` with the shape slot of every proxy overlapping `aabb`, in the static,
    /// kinematic, then dynamic tree, until it returns false.
    pub(crate) fn query_all<F>(&self, aabb: &Aabb, mut callback: F)
    where
        F: FnMut(u32) -> bool,
    {
        for body_type in [BodyType::Static, BodyType::Kinematic, BodyType::Dynamic] {
            if !self.query_tree(body_type, aabb, &mut callback) {
                return;
            }
        }
    }

    /// Casts a segment through the trees. The callback receives each candidate shape slot and the
    /// current maximum fraction and returns a new fraction; the maximum carries from tree to tree.
    pub(crate) fn ray_cast<F>(
        &self,
        origin: Vec3,
        translation: Vec3,
        max_fraction: f32,
        mut callback: F,
    ) -> f32
    where
        F: FnMut(u32, f32) -> f32,
    {
        let mut max_fraction = max_fraction;
        for tree in &self.trees {
            max_fraction = tree.ray_cast(origin, translation, max_fraction, |node, fraction| {
                callback(tree.user_data(node), fraction)
            });
        }
        max_fraction
    }

    /// Queues a shape whose proxy moved.
    pub(crate) fn mark_moved(&mut self, shape: ShapeId) {
        self.move_buffer.push(shape);
    }
}
