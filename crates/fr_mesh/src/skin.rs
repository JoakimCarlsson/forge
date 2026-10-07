//! A skin: the joints of a mesh as nodes of a transform hierarchy, with inverse bind matrices.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use fr_math::Mat4;
use fr_transform::{Hierarchy, Transform};

/// Why a set of joints is not a valid [`Skin`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkinError {
    /// The joint list and the inverse bind matrix list differ in length.
    LengthMismatch {
        /// The number of joints.
        joints: usize,
        /// The number of inverse bind matrices.
        matrices: usize,
    },
    /// A joint names a node the hierarchy does not have.
    UnknownNode {
        /// The position of the joint in the skin.
        joint: usize,
        /// The node index it names.
        node: usize,
    },
    /// Two joints name the same node.
    DuplicateJoint {
        /// The node both name.
        node: usize,
    },
    /// A name is not the name of a node of the hierarchy.
    UnknownName {
        /// The name that was not found.
        name: String,
    },
}

impl fmt::Display for SkinError {
    /// Writes which joint breaks the rules and how.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch { joints, matrices } => {
                write!(
                    f,
                    "{joints} joints come with {matrices} inverse bind matrices"
                )
            }
            Self::UnknownNode { joint, node } => {
                write!(f, "joint {joint} names node {node}, which does not exist")
            }
            Self::DuplicateJoint { node } => write!(f, "more than one joint is node {node}"),
            Self::UnknownName { name } => write!(f, "the hierarchy has no node named {name:?}"),
        }
    }
}

impl Error for SkinError {}

/// The joints of a skin: indices of nodes of a [`Hierarchy`], in the order the skin lists them,
/// each with the matrix that takes a model-space point into the joint's bind space.
#[derive(Clone, Debug, PartialEq)]
pub struct Skin {
    /// The node of each joint.
    joints: Vec<usize>,
    /// The inverse bind matrix of each joint.
    inverse_bind: Vec<Mat4>,
}

impl Skin {
    /// A skin of the nodes `joints` of `hierarchy` with explicit inverse bind matrices.
    ///
    /// # Errors
    ///
    /// Returns [`SkinError`] when the lists differ in length, a joint is not a node of the
    /// hierarchy or a node is listed twice.
    pub fn new(
        hierarchy: &Hierarchy,
        joints: Vec<usize>,
        inverse_bind: Vec<Mat4>,
    ) -> Result<Self, SkinError> {
        if joints.len() != inverse_bind.len() {
            return Err(SkinError::LengthMismatch {
                joints: joints.len(),
                matrices: inverse_bind.len(),
            });
        }
        let mut seen = HashSet::new();
        for (joint, &node) in joints.iter().enumerate() {
            if node >= hierarchy.len() {
                return Err(SkinError::UnknownNode { joint, node });
            }
            if !seen.insert(node) {
                return Err(SkinError::DuplicateJoint { node });
            }
        }
        Ok(Self {
            joints,
            inverse_bind,
        })
    }

    /// A skin of the nodes `joints` of `hierarchy` whose inverse bind matrices are the
    /// inverses of the nodes' bind world poses.
    ///
    /// # Errors
    ///
    /// Returns [`SkinError`] when a joint is not a node of the hierarchy or a node is
    /// listed twice.
    pub fn from_bind_pose(hierarchy: &Hierarchy, joints: Vec<usize>) -> Result<Self, SkinError> {
        let bind = hierarchy.bind_world();
        let inverse_bind = joints
            .iter()
            .map(|&node| {
                bind.get(node)
                    .map_or(Mat4::IDENTITY, |pose| pose.matrix().inverse())
            })
            .collect();
        Self::new(hierarchy, joints, inverse_bind)
    }

    /// A skin of the nodes of `hierarchy` called `names`, in that order, with bind pose
    /// inverse bind matrices.
    ///
    /// # Errors
    ///
    /// Returns [`SkinError`] when a name is not a node of the hierarchy or is repeated.
    pub fn from_names(hierarchy: &Hierarchy, names: &[&str]) -> Result<Self, SkinError> {
        let joints = names
            .iter()
            .map(|&name| {
                hierarchy.find(name).ok_or_else(|| SkinError::UnknownName {
                    name: name.to_owned(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_bind_pose(hierarchy, joints)
    }

    /// The number of joints.
    pub fn len(&self) -> usize {
        self.joints.len()
    }

    /// Whether there are no joints.
    pub fn is_empty(&self) -> bool {
        self.joints.is_empty()
    }

    /// The node of each joint, in skin order.
    pub fn joints(&self) -> &[usize] {
        &self.joints
    }

    /// The inverse bind matrix of each joint, in skin order.
    pub fn inverse_bind_matrices(&self) -> &[Mat4] {
        &self.inverse_bind
    }

    /// The name of each joint in `hierarchy`, in skin order.
    pub fn joint_names<'a>(&self, hierarchy: &'a Hierarchy) -> Vec<&'a str> {
        self.joints
            .iter()
            .filter_map(|&node| hierarchy.name(node))
            .collect()
    }

    /// The matrix of each joint that skins a mesh: the node's pose in `world`, a slice of world
    /// poses indexed by node, times its inverse bind matrix.
    ///
    /// A node with no entry in `world` contributes the identity.
    pub fn joint_matrices(&self, world: &[Transform]) -> Vec<Mat4> {
        self.joints
            .iter()
            .zip(&self.inverse_bind)
            .map(|(&node, inverse_bind)| {
                world
                    .get(node)
                    .map_or(Mat4::IDENTITY, |pose| pose.matrix() * *inverse_bind)
            })
            .collect()
    }
}
