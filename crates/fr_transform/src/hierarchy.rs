//! A tree of named local transforms: the bones of a rig, the nodes of a model.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::transform::Transform;

/// One node of a [`Hierarchy`] as it is handed in.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// The unique name of the node.
    pub name: String,
    /// The index of the parent node, which is always smaller than this node's own index, or
    /// `None` for a root.
    pub parent: Option<usize>,
    /// The pose relative to the parent; it becomes the bind pose and the current pose.
    pub local: Transform,
}

impl Node {
    /// A node called `name` under `parent` at `local`.
    pub fn new(name: impl Into<String>, parent: Option<usize>, local: Transform) -> Self {
        Self {
            name: name.into(),
            parent,
            local,
        }
    }
}

/// Why a list of nodes is not a valid [`Hierarchy`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HierarchyError {
    /// A node's parent does not come before it in the list.
    ParentNotBefore {
        /// The index of the offending node.
        node: usize,
        /// The parent index it names.
        parent: usize,
    },
    /// Two nodes share a name.
    DuplicateName {
        /// The repeated name.
        name: String,
    },
}

impl fmt::Display for HierarchyError {
    /// Writes which node breaks the rules and how.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParentNotBefore { node, parent } => {
                write!(
                    f,
                    "node {node} names parent {parent}, which does not precede it"
                )
            }
            Self::DuplicateName { name } => write!(f, "more than one node is named {name:?}"),
        }
    }
}

impl Error for HierarchyError {}

/// A tree of named local transforms in which every parent precedes its children.
///
/// Each node has a bind pose, the rest pose it was created with, and a current pose that can be
/// changed. World poses compose the local poses down the tree; scale composes component-wise, so
/// shear is not represented.
#[derive(Clone, Debug, PartialEq)]
pub struct Hierarchy {
    /// The names of the nodes, parents first.
    names: Vec<String>,
    /// The parent of each node.
    parents: Vec<Option<usize>>,
    /// The rest pose of each node relative to its parent.
    bind: Vec<Transform>,
    /// The current pose of each node relative to its parent.
    local: Vec<Transform>,
}

/// The pose of a child with pose `local` inside `parent`, in the parent's space.
fn compose(parent: &Transform, local: &Transform) -> Transform {
    Transform {
        translation: parent.translation + parent.rotation * (parent.scale * local.translation),
        rotation: (parent.rotation * local.rotation).normalize(),
        scale: parent.scale * local.scale,
    }
}

/// The pose of `world` relative to `parent`, the inverse of [`compose`].
fn relative(parent: &Transform, world: &Transform) -> Transform {
    let inverse = parent.rotation.inverse();
    Transform {
        translation: (inverse * (world.translation - parent.translation)) / parent.scale,
        rotation: (inverse * world.rotation).normalize(),
        scale: world.scale / parent.scale,
    }
}

impl Hierarchy {
    /// Builds a hierarchy from `nodes`, parents first, with every current pose at the bind pose.
    ///
    /// # Errors
    ///
    /// Returns [`HierarchyError`] when a parent index is not smaller than the node's own index
    /// or when two nodes share a name.
    pub fn new(nodes: Vec<Node>) -> Result<Self, HierarchyError> {
        let mut seen = HashSet::new();
        for (index, node) in nodes.iter().enumerate() {
            if let Some(parent) = node.parent.filter(|&parent| parent >= index) {
                return Err(HierarchyError::ParentNotBefore {
                    node: index,
                    parent,
                });
            }
            if !seen.insert(node.name.as_str()) {
                return Err(HierarchyError::DuplicateName {
                    name: node.name.clone(),
                });
            }
        }
        let bind: Vec<Transform> = nodes.iter().map(|node| node.local).collect();
        Ok(Self {
            names: nodes.iter().map(|node| node.name.clone()).collect(),
            parents: nodes.iter().map(|node| node.parent).collect(),
            local: bind.clone(),
            bind,
        })
    }

    /// The number of nodes.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether there are no nodes.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The names of the nodes, parents first.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The name of the node at `index`, if there is one.
    pub fn name(&self, index: usize) -> Option<&str> {
        self.names.get(index).map(String::as_str)
    }

    /// The index of the node called `name`, if there is one.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|candidate| candidate == name)
    }

    /// The parent of the node at `index`, or `None` for a root or an index out of range.
    pub fn parent(&self, index: usize) -> Option<usize> {
        self.parents.get(index).copied().flatten()
    }

    /// The indices of the nodes whose parent is `index`, ascending.
    pub fn children(&self, index: usize) -> Vec<usize> {
        self.parents
            .iter()
            .enumerate()
            .filter(|(_, parent)| **parent == Some(index))
            .map(|(child, _)| child)
            .collect()
    }

    /// The bind pose of every node relative to its parent.
    pub fn bind_locals(&self) -> &[Transform] {
        &self.bind
    }

    /// The current pose of every node relative to its parent.
    pub fn locals(&self) -> &[Transform] {
        &self.local
    }

    /// Moves the node at `index` to `local`; an index out of range is ignored.
    pub fn set_local(&mut self, index: usize, local: Transform) {
        if let Some(slot) = self.local.get_mut(index) {
            *slot = local;
        }
    }

    /// Moves the nodes to `locals`; a node with no entry keeps its pose and extra entries are
    /// ignored.
    pub fn set_locals(&mut self, locals: &[Transform]) {
        for (slot, local) in self.local.iter_mut().zip(locals) {
            *slot = *local;
        }
    }

    /// Moves every node back to its bind pose.
    pub fn reset_to_bind(&mut self) {
        self.local.clone_from(&self.bind);
    }

    /// The world pose of every node at the bind pose.
    pub fn bind_world(&self) -> Vec<Transform> {
        self.world_from(&self.bind)
    }

    /// The world pose of every node at the current pose.
    pub fn world(&self) -> Vec<Transform> {
        self.world_from(&self.local)
    }

    /// The world pose of every node given local poses.
    ///
    /// A node with no entry in `locals` keeps its bind pose; extra entries are ignored.
    pub fn world_from(&self, locals: &[Transform]) -> Vec<Transform> {
        let mut world: Vec<Transform> = Vec::with_capacity(self.len());
        for (index, bind) in self.bind.iter().enumerate() {
            let pose = locals.get(index).unwrap_or(bind);
            let placed = match self.parents[index].and_then(|parent| world.get(parent)) {
                Some(parent) => compose(parent, pose),
                None => *pose,
            };
            world.push(placed);
        }
        world
    }

    /// The local poses that [`Hierarchy::world_from`] turns into `world`.
    ///
    /// A node with no entry in `world` is taken at its bind pose.
    pub fn local_from_world(&self, world: &[Transform]) -> Vec<Transform> {
        let bind = self.bind_world();
        let resolved: Vec<Transform> = bind
            .iter()
            .enumerate()
            .map(|(index, fallback)| world.get(index).copied().unwrap_or(*fallback))
            .collect();
        self.parents
            .iter()
            .zip(&resolved)
            .map(
                |(parent, pose)| match parent.and_then(|parent| resolved.get(parent)) {
                    Some(parent) => relative(parent, pose),
                    None => *pose,
                },
            )
            .collect()
    }

    /// Moves the nodes so that their world poses are `world`; see [`Hierarchy::local_from_world`].
    pub fn set_world(&mut self, world: &[Transform]) {
        self.local = self.local_from_world(world);
    }
}
