//! Dynamic AABB tree with surface-area-heuristic insertion and AVL-style rotations.

use std::cmp::max;

use fr_math::Vec3;

use fr_math::Aabb;

/// Index of a node in the tree; also the proxy id of a leaf.
pub type NodeId = i32;

/// The absent node.
pub const NULL_NODE: NodeId = -1;

/// Initial capacity of the stack of an overlap query.
const QUERY_STACK_CAPACITY: usize = 64;

/// One node of the tree: a leaf holding a proxy or an internal node bounding
/// two children.
#[derive(Clone, Copy, Debug)]
struct Node {
    /// Bounds of the leaf, or of both children.
    aabb: Aabb,
    /// Value stored with a leaf.
    user_data: u32,
    /// Parent node.
    parent: NodeId,
    /// First child; absent for a leaf.
    child1: NodeId,
    /// Second child.
    child2: NodeId,
    /// Height of the subtree; zero for a leaf and minus one when free.
    height: i32,
}

impl Default for Node {
    /// Returns a free-standing leaf with zero bounds and no links.
    fn default() -> Self {
        Self {
            aabb: Aabb::default(),
            user_data: 0,
            parent: NULL_NODE,
            child1: NULL_NODE,
            child2: NULL_NODE,
            height: 0,
        }
    }
}

impl Node {
    /// Tells whether the node is a leaf.
    fn is_leaf(&self) -> bool {
        self.child1 == NULL_NODE
    }
}

/// A dynamic bounding volume tree of leaf proxies.
#[derive(Clone, Debug)]
pub struct Tree {
    /// Node storage; freed nodes stay in place and are reused.
    nodes: Vec<Node>,
    /// Freed node ids, reused last in, first out.
    free_list: Vec<NodeId>,
    /// Root node.
    root: NodeId,
    /// Number of live leaves.
    proxy_count: usize,
}

impl Default for Tree {
    /// Returns an empty tree.
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    /// Creates an empty tree.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            free_list: Vec::new(),
            root: NULL_NODE,
            proxy_count: 0,
        }
    }

    /// Inserts a leaf with the given bounds and user data and returns its id.
    pub fn create_proxy(&mut self, aabb: &Aabb, user_data: u32) -> NodeId {
        let id = self.allocate_node();
        let leaf = self.node_mut(id);
        leaf.aabb = *aabb;
        leaf.user_data = user_data;
        leaf.height = 0;
        self.insert_leaf(id);
        self.proxy_count += 1;
        id
    }

    /// Removes a leaf and frees its node.
    pub fn destroy_proxy(&mut self, proxy: NodeId) {
        self.remove_leaf(proxy);
        self.free_node(proxy);
        self.proxy_count -= 1;
    }

    /// Reinserts a leaf with new bounds.
    pub fn move_proxy(&mut self, proxy: NodeId, aabb: &Aabb) {
        self.remove_leaf(proxy);
        self.node_mut(proxy).aabb = *aabb;
        self.insert_leaf(proxy);
    }

    /// Returns the bounds of a proxy.
    pub fn aabb(&self, proxy: NodeId) -> &Aabb {
        &self.node(proxy).aabb
    }

    /// Returns the user data of a proxy.
    pub fn user_data(&self, proxy: NodeId) -> u32 {
        self.node(proxy).user_data
    }

    /// Returns the number of live proxies.
    pub fn proxy_count(&self) -> usize {
        self.proxy_count
    }

    /// Returns the height of the tree, zero when empty.
    pub fn height(&self) -> i32 {
        if self.root == NULL_NODE {
            0
        } else {
            self.node(self.root).height
        }
    }

    /// Calls `callback` with every leaf whose bounds overlap `aabb`, visiting
    /// the second child before the first, until the callback returns false.
    pub fn query<F>(&self, aabb: &Aabb, mut callback: F)
    where
        F: FnMut(NodeId) -> bool,
    {
        let mut stack: Vec<NodeId> = Vec::with_capacity(QUERY_STACK_CAPACITY);
        stack.push(self.root);
        while let Some(id) = stack.pop() {
            if id == NULL_NODE {
                continue;
            }
            let node = self.node(id);
            if !node.aabb.overlaps(aabb) {
                continue;
            }
            if node.is_leaf() {
                if !callback(id) {
                    return;
                }
            } else {
                stack.push(node.child1);
                stack.push(node.child2);
            }
        }
    }

    /// Walks the tree along a segment, nearest child first. For every leaf the segment reaches,
    /// `callback` receives the leaf and the current maximum fraction and returns a new fraction,
    /// which shortens the segment when it lies in zero to the current maximum. Returns the final
    /// maximum fraction.
    pub fn ray_cast<F>(
        &self,
        origin: Vec3,
        translation: Vec3,
        mut max_fraction: f32,
        mut callback: F,
    ) -> f32
    where
        F: FnMut(NodeId, f32) -> f32,
    {
        let mut stack: Vec<NodeId> = Vec::with_capacity(QUERY_STACK_CAPACITY);
        stack.push(self.root);
        while let Some(id) = stack.pop() {
            if id == NULL_NODE {
                continue;
            }
            let current = *self.node(id);
            if current
                .aabb
                .ray_entry(origin, translation, max_fraction)
                .is_none()
            {
                continue;
            }
            if current.is_leaf() {
                let candidate = callback(id, max_fraction);
                if candidate >= 0.0 && candidate <= max_fraction {
                    max_fraction = candidate;
                }
                if max_fraction == 0.0 {
                    return max_fraction;
                }
                continue;
            }
            let entry1 =
                self.node(current.child1)
                    .aabb
                    .ray_entry(origin, translation, max_fraction);
            let entry2 =
                self.node(current.child2)
                    .aabb
                    .ray_entry(origin, translation, max_fraction);
            match (entry1, entry2) {
                (Some(entry1), Some(entry2)) => {
                    let first_is_nearer = entry1 <= entry2;
                    stack.push(if first_is_nearer {
                        current.child2
                    } else {
                        current.child1
                    });
                    stack.push(if first_is_nearer {
                        current.child1
                    } else {
                        current.child2
                    });
                }
                (Some(_), None) => stack.push(current.child1),
                (None, Some(_)) => stack.push(current.child2),
                (None, None) => {}
            }
        }
        max_fraction
    }

    /// Returns the node with the given id.
    fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// Returns the node with the given id for modification.
    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id as usize]
    }

    /// Takes a node from the free list, or appends one, and resets it.
    fn allocate_node(&mut self) -> NodeId {
        match self.free_list.pop() {
            None => {
                self.nodes.push(Node::default());
                (self.nodes.len() - 1) as NodeId
            }
            Some(id) => {
                *self.node_mut(id) = Node::default();
                id
            }
        }
    }

    /// Resets a node, marks it free with height minus one and pushes it on the
    /// free list.
    fn free_node(&mut self, id: NodeId) {
        *self.node_mut(id) = Node::default();
        self.node_mut(id).height = -1;
        self.free_list.push(id);
    }

    /// Returns the surface area of the bounds of the node that adding a leaf
    /// with `leaf_aabb` below `child` would cost, with `inheritance_cost`
    /// charged to the ancestors.
    fn child_cost(&self, child: NodeId, leaf_aabb: &Aabb, inheritance_cost: f32) -> f32 {
        let c = self.node(child);
        let merged = leaf_aabb.union(&c.aabb).surface_area();
        if c.is_leaf() {
            return merged + inheritance_cost;
        }
        merged - c.aabb.surface_area() + inheritance_cost
    }

    /// Walks down from the root choosing the cheaper child by the surface area
    /// heuristic and returns the sibling the new leaf is paired with.
    fn find_sibling(&self, leaf_aabb: &Aabb) -> NodeId {
        let mut index = self.root;
        while !self.node(index).is_leaf() {
            let child1 = self.node(index).child1;
            let child2 = self.node(index).child2;

            let area = self.node(index).aabb.surface_area();
            let combined_area = self.node(index).aabb.union(leaf_aabb).surface_area();
            let cost = 2.0 * combined_area;
            let inheritance_cost = 2.0 * (combined_area - area);

            let cost1 = self.child_cost(child1, leaf_aabb, inheritance_cost);
            let cost2 = self.child_cost(child2, leaf_aabb, inheritance_cost);

            if cost < cost1 && cost < cost2 {
                break;
            }
            index = if cost1 < cost2 { child1 } else { child2 };
        }
        index
    }

    /// Recomputes the height and bounds of a node from its children.
    fn refit(&mut self, index: NodeId) {
        let child1 = self.node(index).child1;
        let child2 = self.node(index).child2;
        let height = 1 + max(self.node(child1).height, self.node(child2).height);
        let aabb = self.node(child1).aabb.union(&self.node(child2).aabb);
        let node = self.node_mut(index);
        node.height = height;
        node.aabb = aabb;
    }

    /// Rebalances and refits every node from `start` up to the root.
    fn rebalance_upwards(&mut self, start: NodeId) {
        let mut index = start;
        while index != NULL_NODE {
            index = self.balance(index);
            self.refit(index);
            index = self.node(index).parent;
        }
    }

    /// Links a leaf into the tree beside its best sibling and rebalances.
    fn insert_leaf(&mut self, leaf: NodeId) {
        if self.root == NULL_NODE {
            self.root = leaf;
            self.node_mut(leaf).parent = NULL_NODE;
            return;
        }

        let leaf_aabb = self.node(leaf).aabb;
        let sibling = self.find_sibling(&leaf_aabb);
        let old_parent = self.node(sibling).parent;
        let new_parent = self.allocate_node();
        let sibling_aabb = self.node(sibling).aabb;
        let sibling_height = self.node(sibling).height;
        let parent = self.node_mut(new_parent);
        parent.parent = old_parent;
        parent.aabb = leaf_aabb.union(&sibling_aabb);
        parent.height = sibling_height + 1;

        if old_parent != NULL_NODE {
            if self.node(old_parent).child1 == sibling {
                self.node_mut(old_parent).child1 = new_parent;
            } else {
                self.node_mut(old_parent).child2 = new_parent;
            }
        } else {
            self.root = new_parent;
        }
        self.node_mut(new_parent).child1 = sibling;
        self.node_mut(new_parent).child2 = leaf;
        self.node_mut(sibling).parent = new_parent;
        self.node_mut(leaf).parent = new_parent;

        let start = self.node(leaf).parent;
        self.rebalance_upwards(start);
    }

    /// Unlinks a leaf from the tree, frees its parent and rebalances.
    fn remove_leaf(&mut self, leaf: NodeId) {
        if leaf == self.root {
            self.root = NULL_NODE;
            return;
        }
        let parent = self.node(leaf).parent;
        let grand_parent = self.node(parent).parent;
        let sibling = if self.node(parent).child1 == leaf {
            self.node(parent).child2
        } else {
            self.node(parent).child1
        };

        if grand_parent != NULL_NODE {
            if self.node(grand_parent).child1 == parent {
                self.node_mut(grand_parent).child1 = sibling;
            } else {
                self.node_mut(grand_parent).child2 = sibling;
            }
            self.node_mut(sibling).parent = grand_parent;
            self.free_node(parent);
            self.rebalance_upwards(grand_parent);
        } else {
            self.root = sibling;
            self.node_mut(sibling).parent = NULL_NODE;
            self.free_node(parent);
        }
    }

    /// Replaces `old_child` of `parent` with `new_child`, or makes `new_child`
    /// the root when `parent` is absent.
    fn replace_child(&mut self, parent: NodeId, old_child: NodeId, new_child: NodeId) {
        if parent == NULL_NODE {
            self.root = new_child;
        } else if self.node(parent).child1 == old_child {
            self.node_mut(parent).child1 = new_child;
        } else {
            self.node_mut(parent).child2 = new_child;
        }
    }

    /// Rotates node `a` below its taller second child `c` and returns `c`.
    fn rotate_second_up(&mut self, a_id: NodeId) -> NodeId {
        let b_id = self.node(a_id).child1;
        let c_id = self.node(a_id).child2;
        let f_id = self.node(c_id).child1;
        let g_id = self.node(c_id).child2;

        self.node_mut(c_id).child1 = a_id;
        let a_parent = self.node(a_id).parent;
        self.node_mut(c_id).parent = a_parent;
        self.node_mut(a_id).parent = c_id;
        self.replace_child(a_parent, a_id, c_id);

        let (tall, short) = if self.node(f_id).height > self.node(g_id).height {
            (f_id, g_id)
        } else {
            (g_id, f_id)
        };
        self.node_mut(c_id).child2 = tall;
        self.node_mut(a_id).child2 = short;
        self.node_mut(short).parent = a_id;
        let a_aabb = self.node(b_id).aabb.union(&self.node(short).aabb);
        self.node_mut(a_id).aabb = a_aabb;
        let c_aabb = a_aabb.union(&self.node(tall).aabb);
        self.node_mut(c_id).aabb = c_aabb;
        let a_height = 1 + max(self.node(b_id).height, self.node(short).height);
        self.node_mut(a_id).height = a_height;
        let c_height = 1 + max(a_height, self.node(tall).height);
        self.node_mut(c_id).height = c_height;
        c_id
    }

    /// Rotates node `a` below its taller first child `b` and returns `b`.
    fn rotate_first_up(&mut self, a_id: NodeId) -> NodeId {
        let b_id = self.node(a_id).child1;
        let c_id = self.node(a_id).child2;
        let d_id = self.node(b_id).child1;
        let e_id = self.node(b_id).child2;

        self.node_mut(b_id).child1 = a_id;
        let a_parent = self.node(a_id).parent;
        self.node_mut(b_id).parent = a_parent;
        self.node_mut(a_id).parent = b_id;
        self.replace_child(a_parent, a_id, b_id);

        let (tall, short) = if self.node(d_id).height > self.node(e_id).height {
            (d_id, e_id)
        } else {
            (e_id, d_id)
        };
        self.node_mut(b_id).child2 = tall;
        self.node_mut(a_id).child1 = short;
        self.node_mut(short).parent = a_id;
        let a_aabb = self.node(c_id).aabb.union(&self.node(short).aabb);
        self.node_mut(a_id).aabb = a_aabb;
        let b_aabb = a_aabb.union(&self.node(tall).aabb);
        self.node_mut(b_id).aabb = b_aabb;
        let a_height = 1 + max(self.node(c_id).height, self.node(short).height);
        self.node_mut(a_id).height = a_height;
        let b_height = 1 + max(a_height, self.node(tall).height);
        self.node_mut(b_id).height = b_height;
        b_id
    }

    /// Performs one AVL-style rotation at `a_id` when its children differ in
    /// height by more than one, and returns the node now at its position.
    fn balance(&mut self, a_id: NodeId) -> NodeId {
        let a = self.node(a_id);
        if a.is_leaf() || a.height < 2 {
            return a_id;
        }
        let imbalance = self.node(a.child2).height - self.node(a.child1).height;
        if imbalance > 1 {
            return self.rotate_second_up(a_id);
        }
        if imbalance < -1 {
            return self.rotate_first_up(a_id);
        }
        a_id
    }
}
