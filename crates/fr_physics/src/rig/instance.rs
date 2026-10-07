//! A rig in a world: the bodies, shapes and joints a [`RigDef`] makes of a hierarchy, with the
//! motors that drive them and the poses read back into the hierarchy.

use std::f32::consts::PI;
use std::fmt;

use fr_math::{Quat, Vec3};
use fr_transform::{Hierarchy, Transform};

use crate::body::{BodyDef, BodyId};
use crate::distance::{DistanceInput, SimplexCache, shape_distance};
use crate::joint::{
    DistanceJoint, JointDef, JointId, JointKind, JointMotor, RevoluteJoint, SphericalJoint,
    WeldJoint,
};
use crate::math::Pose;
use crate::rig::def::{BoneRef, RigDef, RigJoint, RigJointKind};
use crate::shape::{Filter, ShapeDef};
use crate::world::World;

/// Where and how a rig is placed in a world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigSettings {
    /// Where the origin of the hierarchy is placed.
    pub position: Vec3,
    /// How the hierarchy is turned.
    pub rotation: Quat,
    /// The collision group of every shape of the rig, as in Box3D: shapes of the same positive
    /// group always collide, those of the same negative group never do, and zero or different
    /// groups fall back to category and mask. How the bodies of one rig collide with each other
    /// is the business of [`RigDef`] and does not need a group; the group sets how rigs relate
    /// to each other and to the rest of the world.
    pub group: i32,
}

impl Default for RigSettings {
    /// A rig at the origin, unturned, in no group.
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            group: 0,
        }
    }
}

/// Why a rig could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RigError {
    /// A bone name is not in the hierarchy.
    UnknownBone(String),
    /// A bone index is not in the hierarchy.
    BoneOutOfRange(usize),
    /// Two bodies are on the same bone.
    DuplicateBody {
        /// The index of the bone.
        bone: usize,
    },
    /// A joint or collision pair names a bone that has no body at or above it.
    NoBody {
        /// The index of the bone.
        bone: usize,
    },
    /// A joint connects a body to itself.
    SameBody {
        /// The index of the joint in the definition.
        joint: usize,
    },
    /// The world refused to create a joint.
    JointFailed {
        /// The index of the joint in the definition.
        joint: usize,
    },
}

impl fmt::Display for RigError {
    /// Describes what is wrong with the definition or the hierarchy.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBone(name) => write!(f, "the hierarchy has no bone named {name:?}"),
            Self::BoneOutOfRange(bone) => write!(f, "the hierarchy has no bone {bone}"),
            Self::DuplicateBody { bone } => write!(f, "more than one body is on bone {bone}"),
            Self::NoBody { bone } => write!(f, "bone {bone} has no body at or above it"),
            Self::SameBody { joint } => write!(f, "joint {joint} connects a body to itself"),
            Self::JointFailed { joint } => write!(f, "the world could not create joint {joint}"),
        }
    }
}

impl std::error::Error for RigError {}

/// One body of a rig.
#[derive(Clone, Copy, Debug)]
struct RigBodyState {
    /// The body in the world.
    body: BodyId,
}

/// One joint of a rig.
#[derive(Clone, Copy, Debug)]
struct RigJointState {
    /// The joint in the world.
    joint: JointId,
    /// The index of the body on the parent side.
    parent_body: usize,
    /// The index of the body on the child side.
    child_body: usize,
    /// The bone on the parent side.
    parent_bone: usize,
    /// The bone on the child side.
    child_bone: usize,
    /// The rotation of the joint frame in the frame of the parent body.
    frame_a_rotation: Quat,
    /// The rotation of the joint frame in the frame of the child body.
    frame_b_rotation: Quat,
    /// The motor as last set.
    motor: JointMotor,
}

/// A joint of a definition with its bones and bodies resolved to indices.
#[derive(Clone, Copy, Debug)]
struct ResolvedJoint {
    /// The bone on the parent side.
    parent_bone: usize,
    /// The bone on the child side.
    child_bone: usize,
    /// The index of the body on the parent side.
    parent_body: usize,
    /// The index of the body on the child side.
    child_body: usize,
}

/// A definition resolved against a hierarchy, ready to be built.
struct Plan {
    /// The bone of each body.
    body_bones: Vec<usize>,
    /// The body that carries each bone: the nearest at or above it.
    carriers: Vec<Option<usize>>,
    /// The joints.
    joints: Vec<ResolvedJoint>,
    /// The pairs of bodies the definition lists as never colliding, smaller index first.
    ignored: Vec<(usize, usize)>,
}

/// The spawn data of every bone: its bind pose with the rig placed, and its bind scale.
struct Spawn {
    /// The pose of the rig in the world.
    root: Pose,
    /// The bind pose of each bone in the space of the hierarchy.
    bind: Vec<Pose>,
    /// The bind scale of each bone.
    scales: Vec<Vec3>,
}

/// A rig in a world: its bodies, shapes and joints, built from a [`RigDef`] on a [`Hierarchy`].
///
/// Joints are numbered in the order of the definition. Bones with a body follow it; every other
/// bone follows the body of its nearest ancestor that has one, and a bone with no such ancestor
/// stays where it was at spawn.
#[derive(Clone, Debug)]
pub struct Rig {
    /// The bodies, in the order of the definition.
    bodies: Vec<RigBodyState>,
    /// The joints, in the order of the definition.
    joints: Vec<RigJointState>,
    /// The body that carries each bone.
    carriers: Vec<Option<usize>>,
    /// The pose of each bone in the frame of its body, or in the world for a bone with none.
    offsets: Vec<Pose>,
    /// The scale of each bone.
    scales: Vec<Vec3>,
}

/// The index of the bone `bone` refers to in `hierarchy`.
fn resolve_bone(hierarchy: &Hierarchy, bone: &BoneRef) -> Result<usize, RigError> {
    match bone {
        BoneRef::Name(name) => hierarchy
            .find(name)
            .ok_or_else(|| RigError::UnknownBone(name.clone())),
        BoneRef::Index(index) if *index < hierarchy.len() => Ok(*index),
        BoneRef::Index(index) => Err(RigError::BoneOutOfRange(*index)),
    }
}

/// The index of the body at or above `bone`, the nearest one, given the bone of each body.
fn carrier_of(hierarchy: &Hierarchy, body_bones: &[usize], bone: usize) -> Option<usize> {
    let mut current = Some(bone);
    while let Some(candidate) = current {
        if let Some(body) = body_bones.iter().position(|&b| b == candidate) {
            return Some(body);
        }
        current = hierarchy.parent(candidate);
    }
    None
}

/// The body that carries `bone`, or the error that says it has none.
fn require_carrier(carriers: &[Option<usize>], bone: usize) -> Result<usize, RigError> {
    carriers
        .get(bone)
        .copied()
        .flatten()
        .ok_or(RigError::NoBody { bone })
}

/// The pair `(a, b)` with the smaller index first.
fn ordered(a: usize, b: usize) -> (usize, usize) {
    if a < b { (a, b) } else { (b, a) }
}

/// Resolves the bones of `def` against `hierarchy`.
fn plan(def: &RigDef, hierarchy: &Hierarchy) -> Result<Plan, RigError> {
    let mut body_bones: Vec<usize> = Vec::with_capacity(def.bodies.len());
    for body in &def.bodies {
        let bone = resolve_bone(hierarchy, &body.bone)?;
        if body_bones.contains(&bone) {
            return Err(RigError::DuplicateBody { bone });
        }
        body_bones.push(bone);
    }
    let carriers: Vec<Option<usize>> = (0..hierarchy.len())
        .map(|bone| carrier_of(hierarchy, &body_bones, bone))
        .collect();
    let mut joints = Vec::with_capacity(def.joints.len());
    for (index, joint) in def.joints.iter().enumerate() {
        let parent_bone = resolve_bone(hierarchy, &joint.parent)?;
        let child_bone = resolve_bone(hierarchy, &joint.child)?;
        let parent_body = require_carrier(&carriers, parent_bone)?;
        let child_body = require_carrier(&carriers, child_bone)?;
        if parent_body == child_body {
            return Err(RigError::SameBody { joint: index });
        }
        joints.push(ResolvedJoint {
            parent_bone,
            child_bone,
            parent_body,
            child_body,
        });
    }
    let mut ignored = Vec::with_capacity(def.ignored_pairs.len());
    for (a, b) in &def.ignored_pairs {
        let a = require_carrier(&carriers, resolve_bone(hierarchy, a)?)?;
        let b = require_carrier(&carriers, resolve_bone(hierarchy, b)?)?;
        if a != b {
            ignored.push(ordered(a, b));
        }
    }
    Ok(Plan {
        body_bones,
        carriers,
        joints,
        ignored,
    })
}

/// The rotation that takes the z axis of a joint frame onto `axis`, which is any length.
fn frame_towards(axis: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::Z, axis.normalize_or(Vec3::Z))
}

/// Where the two frames of a joint sit.
struct Frames {
    /// The pose of the frame on the parent body.
    parent: Pose,
    /// The pose of the frame on the child body.
    child: Pose,
}

/// The joint frames of `joint` for bodies at `parent_pose` and `child_pose`.
fn joint_frames(
    joint: &RigJoint,
    resolved: &ResolvedJoint,
    spawn: &Spawn,
    parent_pose: &Pose,
    child_pose: &Pose,
) -> Frames {
    let child_bind = spawn.bind[resolved.child_bone];
    let anchor_child = spawn
        .root
        .transform_point(child_bind.transform_point(joint.anchor));
    let anchor_parent = match joint.kind {
        RigJointKind::Distance { parent_anchor, .. } => spawn
            .root
            .transform_point(spawn.bind[resolved.parent_bone].transform_point(parent_anchor)),
        _ => anchor_child,
    };
    let axis = match joint.kind {
        RigJointKind::Spherical { axis, .. } | RigJointKind::Revolute { axis, .. } => {
            frame_towards(axis)
        }
        RigJointKind::Weld | RigJointKind::Distance { .. } => Quat::IDENTITY,
    };
    let rotation = spawn.root.rotation * child_bind.rotation * axis;
    Frames {
        parent: Pose::new(
            parent_pose.inv_transform_point(anchor_parent),
            parent_pose.rotation.conjugate() * rotation,
        ),
        child: Pose::new(
            child_pose.inv_transform_point(anchor_child),
            child_pose.rotation.conjugate() * rotation,
        ),
    }
}

/// The joint of a rig joint's kind and limits, with its motor and friction set, for frames at
/// the given anchors.
fn joint_kind(
    joint: &RigJoint,
    frames: &Frames,
    parent_pose: &Pose,
    child_pose: &Pose,
) -> JointKind {
    let mut kind = match joint.kind {
        RigJointKind::Spherical { cone, twist, .. } => {
            let (lower, upper) = twist.unwrap_or((0.0, 0.0));
            JointKind::Spherical(SphericalJoint {
                enable_cone_limit: cone.is_some(),
                cone_angle: cone.unwrap_or(0.5 * PI),
                enable_twist_limit: twist.is_some(),
                lower_twist_angle: lower,
                upper_twist_angle: upper,
                ..SphericalJoint::default()
            })
        }
        RigJointKind::Revolute { limits, .. } => {
            let (lower, upper) = limits.unwrap_or((0.0, 0.0));
            JointKind::Revolute(RevoluteJoint {
                enable_limit: limits.is_some(),
                lower_angle: lower.clamp(-PI, PI),
                upper_angle: upper.clamp(-PI, PI),
                ..RevoluteJoint::default()
            })
        }
        RigJointKind::Weld => JointKind::Weld(WeldJoint::default()),
        RigJointKind::Distance { length, limits, .. } => {
            let anchor_parent = parent_pose.transform_point(frames.parent.position);
            let anchor_child = child_pose.transform_point(frames.child.position);
            let defaults = DistanceJoint::default();
            JointKind::Distance(DistanceJoint {
                length: length.unwrap_or_else(|| (anchor_child - anchor_parent).length()),
                enable_limit: limits.is_some(),
                min_length: limits.map_or(defaults.min_length, |(lower, _)| lower),
                max_length: limits.map_or(defaults.max_length, |(_, upper)| upper),
                ..defaults
            })
        }
    };
    kind.set_motor(&joint.motor);
    kind.set_friction(joint.friction);
    kind
}

impl Rig {
    /// Creates the bodies, shapes and joints of `def` in `world`, placing the bind pose of
    /// `hierarchy` as `settings` says.
    ///
    /// Every body starts at rest at the bind pose of its bone, every joint's motor drives
    /// towards the bind pose, and nothing is created when an error is returned.
    ///
    /// # Errors
    ///
    /// Returns [`RigError`] when the definition names a bone the hierarchy lacks, puts two
    /// bodies on a bone, joins a body to itself or names a bone that has no body at or above it.
    pub fn build(
        world: &mut World,
        def: &RigDef,
        hierarchy: &Hierarchy,
        settings: &RigSettings,
    ) -> Result<Self, RigError> {
        let plan = plan(def, hierarchy)?;
        let bind_world = hierarchy.bind_world();
        let spawn = Spawn {
            root: Pose::new(settings.position, settings.rotation),
            bind: bind_world.iter().map(Pose::from_transform).collect(),
            scales: bind_world.iter().map(|pose| pose.scale).collect(),
        };
        let filter = Filter {
            group: settings.group,
            ..Filter::default()
        };
        let mut rig = Self {
            bodies: Vec::with_capacity(def.bodies.len()),
            joints: Vec::with_capacity(def.joints.len()),
            carriers: plan.carriers.clone(),
            offsets: Vec::new(),
            scales: spawn.scales.clone(),
        };
        let body_poses = rig.create_bodies(world, def, &plan, &spawn, &filter);
        rig.offsets = (0..hierarchy.len())
            .map(|bone| match plan.carriers[bone] {
                Some(body) => {
                    let body_bind = spawn.bind[plan.body_bones[body]];
                    body_bind.inv_mul(&spawn.bind[bone])
                }
                None => spawn.root.mul(&spawn.bind[bone]),
            })
            .collect();
        if let Err(error) = rig.create_joints(world, def, &plan, &spawn, &body_poses) {
            rig.destroy(world);
            return Err(error);
        }
        rig.disable_self_collisions(world, def, &plan, &body_poses);
        Ok(rig)
    }

    /// Creates the bodies with their shapes and returns their spawn poses.
    fn create_bodies(
        &mut self,
        world: &mut World,
        def: &RigDef,
        plan: &Plan,
        spawn: &Spawn,
        filter: &Filter,
    ) -> Vec<Pose> {
        let mut poses = Vec::with_capacity(def.bodies.len());
        for (body_def, &bone) in def.bodies.iter().zip(&plan.body_bones) {
            let pose = spawn.root.mul(&spawn.bind[bone]);
            let body = world.create_body(&BodyDef::dynamic(pose.position).rotated(pose.rotation));
            for shape in &body_def.shapes {
                let shape_def = ShapeDef::new(shape.geometry.clone())
                    .with_density(shape.density)
                    .with_material(shape.material)
                    .with_filter(*filter);
                world.create_shape(body, &shape_def);
            }
            self.bodies.push(RigBodyState { body });
            poses.push(pose);
        }
        poses
    }

    /// Creates the joints of the definition between the bodies.
    fn create_joints(
        &mut self,
        world: &mut World,
        def: &RigDef,
        plan: &Plan,
        spawn: &Spawn,
        body_poses: &[Pose],
    ) -> Result<(), RigError> {
        for (index, (joint, resolved)) in def.joints.iter().zip(&plan.joints).enumerate() {
            let parent_pose = body_poses[resolved.parent_body];
            let child_pose = body_poses[resolved.child_body];
            let frames = joint_frames(joint, resolved, spawn, &parent_pose, &child_pose);
            let kind = joint_kind(joint, &frames, &parent_pose, &child_pose);
            let mut joint_def = JointDef::new(
                self.bodies[resolved.parent_body].body,
                self.bodies[resolved.child_body].body,
                frames.parent,
                frames.child,
                kind,
            );
            joint_def.collide_connected = joint.collide_connected;
            let id = world
                .create_joint(joint_def)
                .ok_or(RigError::JointFailed { joint: index })?;
            self.joints.push(RigJointState {
                joint: id,
                parent_body: resolved.parent_body,
                child_body: resolved.child_body,
                parent_bone: resolved.parent_bone,
                child_bone: resolved.child_bone,
                frame_a_rotation: frames.parent.rotation,
                frame_b_rotation: frames.child.rotation,
                motor: joint.motor,
            });
        }
        Ok(())
    }

    /// Makes the pairs of bodies that must not collide ignore each other: the pairs the
    /// definition lists, every pair when self collision is off, and the pairs no joint connects
    /// whose shapes are within the rest overlap margin at the spawn poses.
    fn disable_self_collisions(
        &self,
        world: &mut World,
        def: &RigDef,
        plan: &Plan,
        body_poses: &[Pose],
    ) {
        for first in 0..self.bodies.len() {
            for second in (first + 1)..self.bodies.len() {
                let listed = plan.ignored.contains(&(first, second));
                let connected = self
                    .joints
                    .iter()
                    .any(|joint| ordered(joint.parent_body, joint.child_body) == (first, second));
                let ignore = listed
                    || (!connected
                        && (!def.self_collision
                            || self.overlap_at_rest(
                                world,
                                (first, second),
                                body_poses,
                                def.rest_overlap_margin,
                            )));
                if ignore {
                    world.disable_collision_between(
                        self.bodies[first].body,
                        self.bodies[second].body,
                    );
                }
            }
        }
    }

    /// Whether any shape of one body of `pair` is within `margin` of any shape of the other
    /// when the bodies are at `body_poses`.
    fn overlap_at_rest(
        &self,
        world: &World,
        pair: (usize, usize),
        body_poses: &[Pose],
        margin: f32,
    ) -> bool {
        let relative = body_poses[pair.0].inv_mul(&body_poses[pair.1]);
        let shapes_a = world.body_shapes(self.bodies[pair.0].body);
        let shapes_b = world.body_shapes(self.bodies[pair.1].body);
        shapes_a.iter().any(|&shape_a| {
            shapes_b.iter().any(|&shape_b| {
                let (Some(geometry_a), Some(geometry_b)) =
                    (world.shape_geometry(shape_a), world.shape_geometry(shape_b))
                else {
                    return false;
                };
                let input = DistanceInput {
                    proxy_a: geometry_a.proxy(),
                    proxy_b: geometry_b.proxy(),
                    transform: relative,
                    use_radii: true,
                };
                shape_distance(&input, &mut SimplexCache::default()).distance < margin
            })
        })
    }

    /// The bodies of the rig, in the order of the definition.
    pub fn bodies(&self) -> impl Iterator<Item = BodyId> + '_ {
        self.bodies.iter().map(|state| state.body)
    }

    /// The body that carries `bone`: the one on it or on its nearest ancestor that has one.
    pub fn body_of_bone(&self, bone: usize) -> Option<BodyId> {
        let body = self.carriers.get(bone).copied().flatten()?;
        self.bodies.get(body).map(|state| state.body)
    }

    /// The number of bones of the hierarchy the rig was built on.
    pub fn bone_count(&self) -> usize {
        self.offsets.len()
    }

    /// The number of joints of the definition.
    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    /// The joint at `index` of the definition's joint list.
    pub fn joint(&self, index: usize) -> Option<JointId> {
        self.joints.get(index).map(|state| state.joint)
    }

    /// The motor of the joint at `index` as last set.
    pub fn motor(&self, index: usize) -> Option<JointMotor> {
        self.joints.get(index).map(|state| state.motor)
    }

    /// Wakes the bodies on both sides of the joint at `index`.
    fn wake_joint(&self, world: &mut World, index: usize) {
        if let Some(state) = self.joints.get(index) {
            world.wake_body(self.bodies[state.parent_body].body);
            world.wake_body(self.bodies[state.child_body].body);
        }
    }

    /// Replaces the motor of the joint at `index` with `edit` applied to it.
    fn edit_motor(&mut self, world: &mut World, index: usize, edit: &dyn Fn(&mut JointMotor)) {
        let Some(state) = self.joints.get_mut(index) else {
            return;
        };
        edit(&mut state.motor);
        let motor = state.motor;
        let joint = state.joint;
        if let Some(kind) = world.joint_mut(joint) {
            kind.set_motor(&motor);
        }
        self.wake_joint(world, index);
    }

    /// Replaces the motor of every joint with `edit` applied to it.
    fn edit_all_motors(&mut self, world: &mut World, edit: &dyn Fn(&mut JointMotor)) {
        for index in 0..self.joints.len() {
            self.edit_motor(world, index, edit);
        }
    }

    /// Sets the motor of the joint at `index`.
    pub fn set_motor(&mut self, world: &mut World, index: usize, motor: JointMotor) {
        self.edit_motor(world, index, &|current| *current = motor);
    }

    /// Sets the spring stiffness in hertz of the joint at `index`; zero makes it limp.
    pub fn set_motor_strength(&mut self, world: &mut World, index: usize, strength: f32) {
        self.edit_motor(world, index, &|motor| motor.strength = strength);
    }

    /// Sets the damping ratio of the spring of the joint at `index`.
    pub fn set_motor_damping(&mut self, world: &mut World, index: usize, damping: f32) {
        self.edit_motor(world, index, &|motor| motor.damping = damping);
    }

    /// Sets the largest torque the spring of the joint at `index` may apply.
    pub fn set_motor_torque_limit(&mut self, world: &mut World, index: usize, max_torque: f32) {
        self.edit_motor(world, index, &|motor| motor.max_torque = max_torque);
    }

    /// Sets the motor of every joint.
    pub fn set_all_motors(&mut self, world: &mut World, motor: JointMotor) {
        self.edit_all_motors(world, &|current| *current = motor);
    }

    /// Sets the spring stiffness in hertz of every joint; zero makes the rig limp.
    pub fn set_all_motor_strengths(&mut self, world: &mut World, strength: f32) {
        self.edit_all_motors(world, &|motor| motor.strength = strength);
    }

    /// Sets the damping ratio of the spring of every joint.
    pub fn set_all_motor_dampings(&mut self, world: &mut World, damping: f32) {
        self.edit_all_motors(world, &|motor| motor.damping = damping);
    }

    /// Sets the largest torque the spring of every joint may apply.
    pub fn set_all_motor_torque_limits(&mut self, world: &mut World, max_torque: f32) {
        self.edit_all_motors(world, &|motor| motor.max_torque = max_torque);
    }

    /// Sets the joint friction, the torque that resists motion, of the joint at `index`.
    pub fn set_friction(&self, world: &mut World, index: usize, torque: f32) {
        let Some(state) = self.joints.get(index) else {
            return;
        };
        if let Some(kind) = world.joint_mut(state.joint) {
            kind.set_friction(torque);
        }
        self.wake_joint(world, index);
    }

    /// Sets the rotation the joint at `index` drives towards, as the rotation of its frame on
    /// the child body in its frame on the parent body; the identity is the bind pose.
    pub fn set_target_rotation(&self, world: &mut World, index: usize, relative: Quat) {
        let Some(state) = self.joints.get(index) else {
            return;
        };
        if let Some(kind) = world.joint_mut(state.joint) {
            kind.set_target_rotation(relative);
        }
        self.wake_joint(world, index);
    }

    /// Makes every joint drive towards the bind pose.
    pub fn reset_target_pose(&self, world: &mut World) {
        for index in 0..self.joints.len() {
            self.set_target_rotation(world, index, Quat::IDENTITY);
        }
    }

    /// Makes every joint drive towards the pose `locals`, local poses of the nodes of
    /// `hierarchy`, such as a frame of an animation.
    ///
    /// Only the rotation of one bone relative to the other matters, so the pose may be given in
    /// any orientation. A node with no entry in `locals` is taken at its bind pose.
    pub fn set_target_pose(&self, world: &mut World, hierarchy: &Hierarchy, locals: &[Transform]) {
        let poses = hierarchy.world_from(locals);
        for (index, state) in self.joints.iter().enumerate() {
            let (Some(parent), Some(child)) =
                (poses.get(state.parent_bone), poses.get(state.child_bone))
            else {
                continue;
            };
            let parent_offset = self.offsets[state.parent_bone].rotation;
            let child_offset = self.offsets[state.child_bone].rotation;
            let relative = state.frame_a_rotation.conjugate()
                * parent_offset
                * parent.rotation.conjugate()
                * child.rotation
                * child_offset.conjugate()
                * state.frame_b_rotation;
            self.set_target_rotation(world, index, relative.normalize());
        }
    }

    /// Gives every body the linear velocity `velocity`.
    pub fn set_velocity(&self, world: &mut World, velocity: Vec3) {
        for body in self.bodies() {
            world.set_body_linear_velocity(body, velocity);
        }
    }

    /// The world pose of every bone, read from the bodies.
    pub fn bone_world_poses(&self, world: &World) -> Vec<Transform> {
        self.pose_bones(|body| world.body_pose(body))
    }

    /// The world pose of every bone blended between the last two steps.
    pub fn bone_interpolated_poses(&self, world: &World, alpha: f32) -> Vec<Transform> {
        self.pose_bones(|body| world.body_interpolated_pose(body, alpha))
    }

    /// The local pose of every bone relative to its parent in `hierarchy`, the pose to skin a
    /// mesh with.
    pub fn bone_local_poses(&self, world: &World, hierarchy: &Hierarchy) -> Vec<Transform> {
        hierarchy.local_from_world(&self.bone_world_poses(world))
    }

    /// Moves the nodes of `hierarchy` to the poses of the bodies.
    pub fn write_world_poses(&self, world: &World, hierarchy: &mut Hierarchy) {
        hierarchy.set_world(&self.bone_world_poses(world));
    }

    /// Moves the nodes of `hierarchy` to the poses of the bodies blended between the last two
    /// steps.
    pub fn write_interpolated_poses(&self, world: &World, hierarchy: &mut Hierarchy, alpha: f32) {
        hierarchy.set_world(&self.bone_interpolated_poses(world, alpha));
    }

    /// The bone poses given the pose of each body.
    fn pose_bones<F>(&self, body_pose: F) -> Vec<Transform>
    where
        F: Fn(BodyId) -> Option<Pose>,
    {
        (0..self.offsets.len())
            .map(|bone| {
                let pose = match self.carriers[bone] {
                    Some(body) => body_pose(self.bodies[body].body)
                        .unwrap_or(Pose::IDENTITY)
                        .mul(&self.offsets[bone]),
                    None => self.offsets[bone],
                };
                pose.to_transform().with_scale(self.scales[bone])
            })
            .collect()
    }

    /// Destroys the joints and bodies of the rig.
    pub fn destroy(self, world: &mut World) {
        for state in &self.joints {
            world.destroy_joint(state.joint);
        }
        for state in &self.bodies {
            world.destroy_body(state.body);
        }
    }
}
