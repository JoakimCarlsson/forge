//! A bone hierarchy and the pose math over it.

use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use glam::Vec3;

use crate::transform::Transform;

/// Names of the bones of [`Skeleton::humanoid`].
pub mod humanoid {
    /// The root bone at the pelvis.
    pub const HIPS: &str = "hips";
    /// The lower spine.
    pub const SPINE: &str = "spine";
    /// The upper spine and ribcage.
    pub const CHEST: &str = "chest";
    /// The neck.
    pub const NECK: &str = "neck";
    /// The head.
    pub const HEAD: &str = "head";
    /// The character's left upper arm.
    pub const UPPER_ARM_L: &str = "upper_arm.l";
    /// The character's left forearm.
    pub const LOWER_ARM_L: &str = "lower_arm.l";
    /// The character's left hand.
    pub const HAND_L: &str = "hand.l";
    /// The character's right upper arm.
    pub const UPPER_ARM_R: &str = "upper_arm.r";
    /// The character's right forearm.
    pub const LOWER_ARM_R: &str = "lower_arm.r";
    /// The character's right hand.
    pub const HAND_R: &str = "hand.r";
    /// The character's left thigh.
    pub const UPPER_LEG_L: &str = "upper_leg.l";
    /// The character's left shin.
    pub const LOWER_LEG_L: &str = "lower_leg.l";
    /// The character's left foot.
    pub const FOOT_L: &str = "foot.l";
    /// The character's right thigh.
    pub const UPPER_LEG_R: &str = "upper_leg.r";
    /// The character's right shin.
    pub const LOWER_LEG_R: &str = "lower_leg.r";
    /// The character's right foot.
    pub const FOOT_R: &str = "foot.r";
}

/// One joint of a [`Skeleton`].
#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    /// The unique name of the bone.
    pub name: String,
    /// The index of the parent bone, which is always smaller than this bone's
    /// own index, or `None` for a root.
    pub parent: Option<usize>,
    /// The bind pose relative to the parent; only translation and rotation are used.
    pub bind_local: Transform,
}

/// Why a list of bones is not a valid [`Skeleton`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkeletonError {
    /// A bone's parent does not come before it in the list.
    ParentNotBefore {
        /// The index of the offending bone.
        bone: usize,
        /// The parent index it names.
        parent: usize,
    },
    /// Two bones share a name.
    DuplicateName {
        /// The repeated name.
        name: String,
    },
}

impl fmt::Display for SkeletonError {
    /// Writes which bone breaks the rules and how.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParentNotBefore { bone, parent } => {
                write!(
                    f,
                    "bone {bone} names parent {parent}, which does not precede it"
                )
            }
            Self::DuplicateName { name } => write!(f, "more than one bone is named {name:?}"),
        }
    }
}

impl Error for SkeletonError {}

/// A hierarchy of bones in which every parent precedes its children.
#[derive(Clone, Debug, PartialEq)]
pub struct Skeleton {
    /// The bones, parents first.
    bones: Vec<Bone>,
}

/// `transform` reduced to translation and rotation.
fn rigid(transform: &Transform) -> Transform {
    Transform {
        translation: transform.translation,
        rotation: transform.rotation,
        scale: Vec3::ONE,
    }
}

/// The pose of a child with pose `local` inside `parent`, in the parent's space.
fn compose(parent: &Transform, local: &Transform) -> Transform {
    Transform {
        translation: parent.translation + parent.rotation * local.translation,
        rotation: (parent.rotation * local.rotation).normalize(),
        scale: Vec3::ONE,
    }
}

/// The pose of `world` relative to `parent`, the inverse of [`compose`].
fn relative(parent: &Transform, world: &Transform) -> Transform {
    let inverse = parent.rotation.inverse();
    Transform {
        translation: inverse * (world.translation - parent.translation),
        rotation: (inverse * world.rotation).normalize(),
        scale: Vec3::ONE,
    }
}

impl Skeleton {
    /// Builds a skeleton from `bones`, parents first.
    ///
    /// # Errors
    ///
    /// Returns [`SkeletonError`] when a parent index is not smaller than the
    /// bone's own index or when two bones share a name.
    pub fn new(bones: Vec<Bone>) -> Result<Self, SkeletonError> {
        let mut names = HashSet::new();
        for (index, bone) in bones.iter().enumerate() {
            if let Some(parent) = bone.parent.filter(|&parent| parent >= index) {
                return Err(SkeletonError::ParentNotBefore {
                    bone: index,
                    parent,
                });
            }
            if !names.insert(bone.name.as_str()) {
                return Err(SkeletonError::DuplicateName {
                    name: bone.name.clone(),
                });
            }
        }
        Ok(Self { bones })
    }

    /// The number of bones.
    pub fn len(&self) -> usize {
        self.bones.len()
    }

    /// Whether there are no bones.
    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// The bones, parents first.
    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }

    /// The bone at `index`, if there is one.
    pub fn bone(&self, index: usize) -> Option<&Bone> {
        self.bones.get(index)
    }

    /// The index of the bone called `name`, if there is one.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|bone| bone.name == name)
    }

    /// The indices of the bones whose parent is `index`, ascending.
    pub fn children(&self, index: usize) -> Vec<usize> {
        self.bones
            .iter()
            .enumerate()
            .filter(|(_, bone)| bone.parent == Some(index))
            .map(|(child, _)| child)
            .collect()
    }

    /// The model-space bind pose of every bone.
    pub fn bind_world(&self) -> Vec<Transform> {
        self.world_poses(&[])
    }

    /// The model-space pose of every bone given local poses.
    ///
    /// A bone with no entry in `local` keeps its bind pose; extra entries are ignored.
    pub fn world_poses(&self, local: &[Transform]) -> Vec<Transform> {
        let mut world: Vec<Transform> = Vec::with_capacity(self.bones.len());
        for (index, bone) in self.bones.iter().enumerate() {
            let pose = local.get(index).unwrap_or(&bone.bind_local);
            let placed = match bone.parent.and_then(|parent| world.get(parent)) {
                Some(parent) => compose(parent, pose),
                None => rigid(pose),
            };
            world.push(placed);
        }
        world
    }

    /// The local poses that [`Skeleton::world_poses`] turns into `world`.
    ///
    /// A bone with no entry in `world` is taken at its bind pose.
    pub fn local_from_world(&self, world: &[Transform]) -> Vec<Transform> {
        let bind = self.bind_world();
        let resolved: Vec<Transform> = bind
            .iter()
            .enumerate()
            .map(|(index, fallback)| world.get(index).map_or(*fallback, rigid))
            .collect();
        self.bones
            .iter()
            .zip(&resolved)
            .map(
                |(bone, pose)| match bone.parent.and_then(|p| resolved.get(p)) {
                    Some(parent) => relative(parent, pose),
                    None => *pose,
                },
            )
            .collect()
    }

    /// A T-pose human in metres standing on `y = 0`, facing `+Z`, whose left
    /// side is `+X`; every bind rotation is the identity.
    pub fn humanoid() -> Self {
        use humanoid::*;
        let joints: [(&str, Option<usize>, Vec3); 17] = [
            (HIPS, None, Vec3::new(0.0, 1.0, 0.0)),
            (SPINE, Some(0), Vec3::new(0.0, 1.1, 0.0)),
            (CHEST, Some(1), Vec3::new(0.0, 1.3, 0.0)),
            (NECK, Some(2), Vec3::new(0.0, 1.55, 0.0)),
            (HEAD, Some(3), Vec3::new(0.0, 1.65, 0.0)),
            (UPPER_ARM_L, Some(2), Vec3::new(0.2, 1.5, 0.0)),
            (LOWER_ARM_L, Some(5), Vec3::new(0.48, 1.5, 0.0)),
            (HAND_L, Some(6), Vec3::new(0.74, 1.5, 0.0)),
            (UPPER_ARM_R, Some(2), Vec3::new(-0.2, 1.5, 0.0)),
            (LOWER_ARM_R, Some(8), Vec3::new(-0.48, 1.5, 0.0)),
            (HAND_R, Some(9), Vec3::new(-0.74, 1.5, 0.0)),
            (UPPER_LEG_L, Some(0), Vec3::new(0.1, 0.95, 0.0)),
            (LOWER_LEG_L, Some(11), Vec3::new(0.1, 0.52, 0.0)),
            (FOOT_L, Some(12), Vec3::new(0.1, 0.08, 0.0)),
            (UPPER_LEG_R, Some(0), Vec3::new(-0.1, 0.95, 0.0)),
            (LOWER_LEG_R, Some(14), Vec3::new(-0.1, 0.52, 0.0)),
            (FOOT_R, Some(15), Vec3::new(-0.1, 0.08, 0.0)),
        ];
        let bones = joints
            .iter()
            .map(|&(name, parent, position)| {
                let parent_position = parent.map_or(Vec3::ZERO, |p| joints[p].2);
                Bone {
                    name: name.to_owned(),
                    parent,
                    bind_local: Transform::from_translation(position - parent_position),
                }
            })
            .collect();
        Self { bones }
    }
}
