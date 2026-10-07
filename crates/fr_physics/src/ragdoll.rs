//! A ragdoll builder: capsule and box bodies for the bone groups of a humanoid [`Skeleton`],
//! joined by spherical joints with cone and twist limits and revolute joints with angle limits,
//! after Box3D's ragdoll sample.
//!
//! Every joint has a motor at zero speed whose maximum torque is the joint friction, and a weak
//! spring towards the bind pose. Bodies of one ragdoll never collide with each other.

use std::f32::consts::PI;
use std::fmt;
use std::sync::Arc;

use fr_core::humanoid as bone;
use fr_core::{Quat, Skeleton, Transform, Vec3};

use crate::body::{BodyDef, BodyId};
use crate::geometry::{Capsule, Geometry};
use crate::hull::Hull;
use crate::joint::{JointDef, JointId, JointKind, RevoluteJoint, SphericalJoint};
use crate::math::Pose;
use crate::shape::{Filter, Material, ShapeDef};
use crate::world::World;

/// The settings of a ragdoll.
#[derive(Clone, Copy, Debug)]
pub struct RagdollDef {
    /// Where the origin of the skeleton is placed.
    pub position: Vec3,
    /// How the skeleton is turned.
    pub rotation: Quat,
    /// A positive number that identifies the ragdoll; its shapes use the negative as collision
    /// group so they never collide with each other.
    pub group: i32,
    /// The torque that joint friction can resist with, before each joint's scale.
    pub friction_torque: f32,
    /// The stiffness of the spring towards the bind pose, in hertz; zero is a limp ragdoll.
    pub joint_hertz: f32,
    /// The damping ratio of the joint springs.
    pub joint_damping_ratio: f32,
    /// The density of the shapes in kilograms per cubic metre.
    pub density: f32,
}

impl Default for RagdollDef {
    /// A ragdoll at the origin with the values of Box3D's ragdoll sample.
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            group: 1,
            friction_torque: 5.0,
            joint_hertz: 1.0,
            joint_damping_ratio: 0.7,
            density: 1000.0,
        }
    }
}

/// Why a ragdoll could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RagdollError {
    /// The skeleton lacks a bone of the humanoid layout.
    MissingBone(&'static str),
}

impl fmt::Display for RagdollError {
    /// Names the missing bone.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBone(name) => write!(f, "the skeleton has no bone named {name:?}"),
        }
    }
}

impl std::error::Error for RagdollError {}

/// One rigid body of a ragdoll with the bones it carries.
#[derive(Clone, Debug)]
struct Part {
    /// The body.
    body: BodyId,
    /// The joint to the parent part, none for the root.
    joint: Option<JointId>,
    /// How much of the friction torque this joint gets.
    friction_scale: f32,
}

/// A ragdoll in a world.
#[derive(Clone, Debug)]
pub struct Ragdoll {
    /// The skeleton the ragdoll was built from.
    skeleton: Skeleton,
    /// The bodies, root first.
    parts: Vec<Part>,
    /// The part each bone follows.
    bone_part: Vec<usize>,
    /// The pose of each bone in the frame of its part's body.
    bone_offsets: Vec<Pose>,
}

/// The two bodies of a joint and where it sits, in the world.
struct Attachment {
    /// The parent body.
    parent: BodyId,
    /// The pose of the parent body at spawn.
    parent_pose: Pose,
    /// The child body.
    child: BodyId,
    /// The pose of the child body at spawn.
    child_pose: Pose,
    /// The joint anchor in the world.
    world_anchor: Vec3,
    /// The orientation of the whole ragdoll.
    root_rotation: Quat,
}

/// The shape of a joint between two parts.
#[derive(Clone, Copy, Debug)]
enum JointShape {
    /// A ball joint with its twist axis along `axis`, a cone limit and twist limits.
    Ball {
        /// The twist axis in the world at the bind pose.
        axis: Vec3,
        /// The cone half angle in degrees.
        cone: f32,
        /// The twist limit either way in degrees.
        twist: f32,
    },
    /// A hinge about `axis` with limits in degrees.
    Hinge {
        /// The hinge axis in the world at the bind pose.
        axis: Vec3,
        /// The lowest angle in degrees.
        lower: f32,
        /// The highest angle in degrees.
        upper: f32,
    },
}

/// The shapes and joint of one part, in bind pose world space.
struct PartSpec {
    /// The bones the part carries; the first one places the body.
    bones: Vec<usize>,
    /// The index of the parent part.
    parent: Option<usize>,
    /// The capsules as two world points and a radius.
    capsules: Vec<(Vec3, Vec3, f32)>,
    /// The boxes as a world centre and half extents.
    boxes: Vec<(Vec3, Vec3)>,
    /// The joint to the parent: anchor in the world, its shape and the friction scale.
    joint: Option<(Vec3, JointShape, f32)>,
}

/// The bind pose world position of the bone called `name`.
fn bone_position(
    skeleton: &Skeleton,
    world: &[Transform],
    name: &'static str,
) -> Result<Vec3, RagdollError> {
    let index = skeleton.find(name).ok_or(RagdollError::MissingBone(name))?;
    Ok(world[index].translation)
}

/// The index of the bone called `name`.
fn bone_index(skeleton: &Skeleton, name: &'static str) -> Result<usize, RagdollError> {
    skeleton.find(name).ok_or(RagdollError::MissingBone(name))
}

/// Builds the part specifications of a humanoid in bind pose world space.
fn humanoid_specs(skeleton: &Skeleton) -> Result<Vec<PartSpec>, RagdollError> {
    let world = skeleton.bind_world();
    let p = |name: &'static str| bone_position(skeleton, &world, name);
    let b = |name: &'static str| bone_index(skeleton, name);
    let spine = p(bone::SPINE)?;
    let chest = p(bone::CHEST)?;
    let neck = p(bone::NECK)?;
    let head = p(bone::HEAD)?;
    let up = Vec3::Y;
    let mut specs = vec![PartSpec {
        bones: vec![b(bone::HIPS)?],
        parent: None,
        capsules: vec![(p(bone::UPPER_LEG_L)?, p(bone::UPPER_LEG_R)?, 0.115)],
        boxes: Vec::new(),
        joint: None,
    }];
    specs.push(PartSpec {
        bones: vec![b(bone::SPINE)?],
        parent: Some(0),
        capsules: vec![(spine, chest, 0.1)],
        boxes: Vec::new(),
        joint: Some((
            spine,
            JointShape::Ball {
                axis: up,
                cone: 25.0,
                twist: 15.0,
            },
            1.0,
        )),
    });
    let box_center = (chest + neck) * 0.5;
    specs.push(PartSpec {
        bones: vec![b(bone::CHEST)?],
        parent: Some(1),
        capsules: Vec::new(),
        boxes: vec![(
            box_center,
            Vec3::new(0.19, 0.5 * (neck.y - chest.y) + 0.02, 0.1),
        )],
        joint: Some((
            chest,
            JointShape::Ball {
                axis: up,
                cone: 25.0,
                twist: 15.0,
            },
            1.0,
        )),
    });
    specs.push(PartSpec {
        bones: vec![b(bone::NECK)?, b(bone::HEAD)?],
        parent: Some(2),
        capsules: vec![(neck, head + up * 0.1, 0.1)],
        boxes: Vec::new(),
        joint: Some((
            neck,
            JointShape::Ball {
                axis: up,
                cone: 40.0,
                twist: 35.0,
            },
            0.8,
        )),
    });
    let arms = [
        (true, bone::UPPER_ARM_L, bone::LOWER_ARM_L, bone::HAND_L),
        (false, bone::UPPER_ARM_R, bone::LOWER_ARM_R, bone::HAND_R),
    ];
    for (left, upper, lower, hand) in arms {
        let shoulder = p(upper)?;
        let elbow = p(lower)?;
        let wrist = p(hand)?;
        let upper_dir = (elbow - shoulder).normalize();
        let lower_dir = (wrist - elbow).normalize();
        let upper_part = specs.len();
        specs.push(PartSpec {
            bones: vec![b(upper)?],
            parent: Some(2),
            capsules: vec![(shoulder, elbow, 0.05)],
            boxes: Vec::new(),
            joint: Some((
                shoulder,
                JointShape::Ball {
                    axis: upper_dir,
                    cone: 80.0,
                    twist: 40.0,
                },
                0.8,
            )),
        });
        let hinge = if left { -Vec3::Y } else { Vec3::Y };
        specs.push(PartSpec {
            bones: vec![b(lower)?, b(hand)?],
            parent: Some(upper_part),
            capsules: vec![(elbow, wrist + lower_dir * 0.12, 0.045)],
            boxes: Vec::new(),
            joint: Some((
                elbow,
                JointShape::Hinge {
                    axis: hinge,
                    lower: 0.0,
                    upper: 140.0,
                },
                0.06,
            )),
        });
    }
    let legs = [
        (bone::UPPER_LEG_L, bone::LOWER_LEG_L, bone::FOOT_L),
        (bone::UPPER_LEG_R, bone::LOWER_LEG_R, bone::FOOT_R),
    ];
    for (upper, lower, foot) in legs {
        let hip = p(upper)?;
        let knee = p(lower)?;
        let ankle = p(foot)?;
        let upper_dir = (knee - hip).normalize();
        let upper_part = specs.len();
        specs.push(PartSpec {
            bones: vec![b(upper)?],
            parent: Some(0),
            capsules: vec![(hip, knee, 0.075)],
            boxes: Vec::new(),
            joint: Some((
                hip,
                JointShape::Ball {
                    axis: upper_dir,
                    cone: 50.0,
                    twist: 20.0,
                },
                1.0,
            )),
        });
        specs.push(PartSpec {
            bones: vec![b(lower)?, b(foot)?],
            parent: Some(upper_part),
            capsules: vec![(knee, ankle, 0.06)],
            boxes: vec![(
                ankle + Vec3::new(0.0, -0.045, 0.06),
                Vec3::new(0.05, 0.035, 0.11),
            )],
            joint: Some((
                knee,
                JointShape::Hinge {
                    axis: Vec3::X,
                    lower: 0.0,
                    upper: 140.0,
                },
                0.5,
            )),
        });
    }
    Ok(specs)
}

impl Ragdoll {
    /// Creates the bodies, shapes and joints of a humanoid ragdoll in `world`.
    ///
    /// # Errors
    ///
    /// Returns [`RagdollError`] when the skeleton lacks a bone of [`fr_core::humanoid`].
    pub fn build(
        world: &mut World,
        skeleton: &Skeleton,
        def: &RagdollDef,
    ) -> Result<Self, RagdollError> {
        let specs = humanoid_specs(skeleton)?;
        let root = Pose::new(def.position, def.rotation);
        let bind = skeleton.bind_world();
        let mut parts: Vec<Part> = Vec::with_capacity(specs.len());
        let mut part_poses: Vec<Pose> = Vec::with_capacity(specs.len());
        let mut bone_part = vec![0; skeleton.len()];
        let mut bone_offsets = vec![Pose::IDENTITY; skeleton.len()];
        let filter = Filter {
            group: -def.group.abs(),
            ..Filter::default()
        };
        let material = Material {
            friction: 0.6,
            rolling_resistance: 0.05,
            ..Material::default()
        };
        for (part_index, spec) in specs.iter().enumerate() {
            let anchor_bind = bind[spec.bones[0]].translation;
            let pose = root.mul(&Pose::from_position(anchor_bind));
            let body = world.create_body(&BodyDef::dynamic(pose.position).rotated(pose.rotation));
            for &(a, b, radius) in &spec.capsules {
                let capsule = Capsule::new(a - anchor_bind, b - anchor_bind, radius);
                let shape = ShapeDef::new(Geometry::Capsule(capsule))
                    .with_density(def.density)
                    .with_material(material)
                    .with_filter(filter);
                world.create_shape(body, &shape);
            }
            for &(center, half) in &spec.boxes {
                let hull =
                    Hull::cuboid(half).transformed(&Pose::from_position(center - anchor_bind));
                let shape = ShapeDef::new(Geometry::Hull(Arc::new(hull)))
                    .with_density(def.density)
                    .with_material(material)
                    .with_filter(filter);
                world.create_shape(body, &shape);
            }
            for &bone_index in &spec.bones {
                bone_part[bone_index] = part_index;
                bone_offsets[bone_index] = Pose::new(
                    bind[bone_index].translation - anchor_bind,
                    bind[bone_index].rotation,
                );
            }
            let joint = match (spec.joint, spec.parent) {
                (Some((anchor, shape, friction_scale)), Some(parent)) => {
                    let attachment = Attachment {
                        parent: parts[parent].body,
                        parent_pose: part_poses[parent],
                        child: body,
                        child_pose: pose,
                        world_anchor: root.transform_point(anchor),
                        root_rotation: def.rotation,
                    };
                    Self::create_joint(world, def, &attachment, shape, friction_scale)
                }
                _ => None,
            };
            parts.push(Part {
                body,
                joint,
                friction_scale: spec.joint.map_or(0.0, |j| j.2),
            });
            part_poses.push(pose);
        }
        Ok(Self {
            skeleton: skeleton.clone(),
            parts,
            bone_part,
            bone_offsets,
        })
    }

    /// Creates the joint between a part and its parent.
    fn create_joint(
        world: &mut World,
        def: &RagdollDef,
        attachment: &Attachment,
        shape: JointShape,
        friction_scale: f32,
    ) -> Option<JointId> {
        let max_motor_torque = friction_scale * def.friction_torque;
        let (frame_rotation, kind) = match shape {
            JointShape::Ball { axis, cone, twist } => {
                let rotation = Quat::from_rotation_arc(Vec3::Z, attachment.root_rotation * axis);
                let joint = SphericalJoint {
                    enable_cone_limit: true,
                    cone_angle: cone.to_radians(),
                    enable_twist_limit: true,
                    lower_twist_angle: -twist.to_radians(),
                    upper_twist_angle: twist.to_radians(),
                    enable_spring: def.joint_hertz > 0.0,
                    hertz: def.joint_hertz,
                    damping_ratio: def.joint_damping_ratio,
                    enable_motor: true,
                    max_motor_torque,
                    ..SphericalJoint::default()
                };
                (rotation, JointKind::Spherical(joint))
            }
            JointShape::Hinge { axis, lower, upper } => {
                let rotation = Quat::from_rotation_arc(Vec3::Z, attachment.root_rotation * axis);
                let joint = RevoluteJoint {
                    enable_limit: true,
                    lower_angle: lower.to_radians().clamp(-PI, PI),
                    upper_angle: upper.to_radians().clamp(-PI, PI),
                    enable_spring: def.joint_hertz > 0.0,
                    hertz: def.joint_hertz,
                    damping_ratio: def.joint_damping_ratio,
                    enable_motor: true,
                    max_motor_torque,
                    ..RevoluteJoint::default()
                };
                (rotation, JointKind::Revolute(joint))
            }
        };
        let frame_a = Pose::new(
            attachment
                .parent_pose
                .inv_transform_point(attachment.world_anchor),
            attachment.parent_pose.rotation.conjugate() * frame_rotation,
        );
        let frame_b = Pose::new(
            attachment
                .child_pose
                .inv_transform_point(attachment.world_anchor),
            attachment.child_pose.rotation.conjugate() * frame_rotation,
        );
        world.create_joint(JointDef::new(
            attachment.parent,
            attachment.child,
            frame_a,
            frame_b,
            kind,
        ))
    }

    /// The bodies of the ragdoll, root first.
    pub fn bodies(&self) -> impl Iterator<Item = BodyId> + '_ {
        self.parts.iter().map(|part| part.body)
    }

    /// The body that carries the bone at `bone`.
    pub fn body_of_bone(&self, bone: usize) -> Option<BodyId> {
        self.bone_part
            .get(bone)
            .and_then(|&part| self.parts.get(part))
            .map(|part| part.body)
    }

    /// The skeleton the ragdoll was built from.
    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// The world pose of every bone, read from the bodies.
    pub fn bone_world_poses(&self, world: &World) -> Vec<Transform> {
        self.pose_bones(|body| world.body_pose(body))
    }

    /// The world pose of every bone blended between the last two steps.
    pub fn bone_interpolated_poses(&self, world: &World, alpha: f32) -> Vec<Transform> {
        self.pose_bones(|body| world.body_interpolated_pose(body, alpha))
    }

    /// The local pose of every bone relative to its parent, the pose to skin a mesh with.
    pub fn bone_local_poses(&self, world: &World) -> Vec<Transform> {
        self.skeleton
            .local_from_world(&self.bone_world_poses(world))
    }

    /// The bone poses given the pose of each body.
    fn pose_bones<F>(&self, body_pose: F) -> Vec<Transform>
    where
        F: Fn(BodyId) -> Option<Pose>,
    {
        (0..self.skeleton.len())
            .map(|bone| {
                let part = &self.parts[self.bone_part[bone]];
                let pose = body_pose(part.body).unwrap_or(Pose::IDENTITY);
                pose.mul(&self.bone_offsets[bone]).to_transform()
            })
            .collect()
    }

    /// Sets the torque that joint friction resists with.
    pub fn set_friction_torque(&self, world: &mut World, torque: f32) {
        for part in &self.parts {
            let Some(joint) = part.joint else {
                continue;
            };
            match world.joint_mut(joint) {
                Some(JointKind::Spherical(joint)) => {
                    joint.max_motor_torque = part.friction_scale * torque;
                }
                Some(JointKind::Revolute(joint)) => {
                    joint.max_motor_torque = part.friction_scale * torque;
                }
                _ => {}
            }
        }
    }

    /// Sets the stiffness and damping of the springs towards the bind pose; zero hertz turns
    /// the springs off.
    pub fn set_spring(&self, world: &mut World, hertz: f32, damping_ratio: f32) {
        for part in &self.parts {
            let Some(joint) = part.joint else {
                continue;
            };
            match world.joint_mut(joint) {
                Some(JointKind::Spherical(joint)) => {
                    joint.enable_spring = hertz > 0.0;
                    joint.hertz = hertz;
                    joint.damping_ratio = damping_ratio;
                }
                Some(JointKind::Revolute(joint)) => {
                    joint.enable_spring = hertz > 0.0;
                    joint.hertz = hertz;
                    joint.damping_ratio = damping_ratio;
                }
                _ => {}
            }
        }
    }

    /// Gives every body a velocity.
    pub fn set_velocity(&self, world: &mut World, velocity: Vec3) {
        for body in self.bodies() {
            world.set_body_linear_velocity(body, velocity);
        }
    }

    /// Destroys the joints and bodies of the ragdoll.
    pub fn destroy(self, world: &mut World) {
        for part in &self.parts {
            if let Some(joint) = part.joint {
                world.destroy_joint(joint);
            }
        }
        for part in &self.parts {
            world.destroy_body(part.body);
        }
    }
}
