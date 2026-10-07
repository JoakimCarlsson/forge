//! The humanoid template: a [`RigDef`] for any hierarchy whose bones map onto the humanoid
//! layout, after Box3D's human sample but with a body for every bone, and a reference T-pose hierarchy to try it on.
//!
//! The layout assumes the bind pose of the hierarchy is an upright T-pose in model space: +Y is
//! up, the character faces +Z and its left side is +X. The names of the bones are looked up in
//! a [`HumanoidBones`] table, so a hierarchy with other names only needs another table.

use std::f32::consts::PI;

use fr_math::Vec3;
use fr_transform::{Hierarchy, HierarchyError, Node, Transform};

use crate::collider::{BoxCollider, CapsuleCollider, SphereCollider};
use crate::joint::JointMotor;
use crate::math::Pose;
use crate::rig::def::{RigBody, RigDef, RigJoint, RigJointKind, RigShapeDef};
use crate::rig::instance::RigError;

/// The spring stiffness of every humanoid joint, in hertz, before it is changed on the rig.
const MOTOR_STRENGTH: f32 = 1.0;

/// The damping ratio of the springs of the humanoid joints.
const MOTOR_DAMPING: f32 = 0.7;

/// The torque limit of the spring of a joint of scale one, in newton metres.
const MOTOR_TORQUE_LIMIT: f32 = 100.0;

/// The joint friction of a joint of scale one, in newton metres.
const FRICTION_TORQUE: f32 = 5.0;

/// The names of the bones of the humanoid layout in a hierarchy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HumanoidBones {
    /// The root bone at the pelvis.
    pub hips: String,
    /// The lower spine.
    pub spine: String,
    /// The upper spine and ribcage.
    pub chest: String,
    /// The neck.
    pub neck: String,
    /// The head.
    pub head: String,
    /// The character's left upper arm.
    pub upper_arm_l: String,
    /// The character's left forearm.
    pub lower_arm_l: String,
    /// The character's left hand.
    pub hand_l: String,
    /// The character's right upper arm.
    pub upper_arm_r: String,
    /// The character's right forearm.
    pub lower_arm_r: String,
    /// The character's right hand.
    pub hand_r: String,
    /// The character's left thigh.
    pub upper_leg_l: String,
    /// The character's left shin.
    pub lower_leg_l: String,
    /// The character's left foot.
    pub foot_l: String,
    /// The character's right thigh.
    pub upper_leg_r: String,
    /// The character's right shin.
    pub lower_leg_r: String,
    /// The character's right foot.
    pub foot_r: String,
}

impl Default for HumanoidBones {
    /// The names `hips`, `spine`, `chest`, `neck`, `head`, `upper_arm.l`, `lower_arm.l`,
    /// `hand.l`, `upper_leg.l`, `lower_leg.l`, `foot.l` and the same ending in `.r`.
    fn default() -> Self {
        let name = |name: &str| name.to_owned();
        Self {
            hips: name("hips"),
            spine: name("spine"),
            chest: name("chest"),
            neck: name("neck"),
            head: name("head"),
            upper_arm_l: name("upper_arm.l"),
            lower_arm_l: name("lower_arm.l"),
            hand_l: name("hand.l"),
            upper_arm_r: name("upper_arm.r"),
            lower_arm_r: name("lower_arm.r"),
            hand_r: name("hand.r"),
            upper_leg_l: name("upper_leg.l"),
            lower_leg_l: name("lower_leg.l"),
            foot_l: name("foot.l"),
            upper_leg_r: name("upper_leg.r"),
            lower_leg_r: name("lower_leg.r"),
            foot_r: name("foot.r"),
        }
    }
}

/// A T-pose human in metres standing on `y = 0`, facing `+Z`, whose left side is `+X`, with
/// bones named by `bones`; every bind rotation is the identity.
///
/// This is a reference hierarchy for [`humanoid_rig`], not part of any rig.
///
/// # Errors
///
/// Returns [`HierarchyError`] when two names of `bones` are the same.
pub fn humanoid_t_pose(bones: &HumanoidBones) -> Result<Hierarchy, HierarchyError> {
    let joints: [(&str, Option<usize>, Vec3); 17] = [
        (&bones.hips, None, Vec3::new(0.0, 1.0, 0.0)),
        (&bones.spine, Some(0), Vec3::new(0.0, 1.1, 0.0)),
        (&bones.chest, Some(1), Vec3::new(0.0, 1.3, 0.0)),
        (&bones.neck, Some(2), Vec3::new(0.0, 1.55, 0.0)),
        (&bones.head, Some(3), Vec3::new(0.0, 1.65, 0.0)),
        (&bones.upper_arm_l, Some(2), Vec3::new(0.2, 1.5, 0.0)),
        (&bones.lower_arm_l, Some(5), Vec3::new(0.48, 1.5, 0.0)),
        (&bones.hand_l, Some(6), Vec3::new(0.74, 1.5, 0.0)),
        (&bones.upper_arm_r, Some(2), Vec3::new(-0.2, 1.5, 0.0)),
        (&bones.lower_arm_r, Some(8), Vec3::new(-0.48, 1.5, 0.0)),
        (&bones.hand_r, Some(9), Vec3::new(-0.74, 1.5, 0.0)),
        (&bones.upper_leg_l, Some(0), Vec3::new(0.1, 0.95, 0.0)),
        (&bones.lower_leg_l, Some(11), Vec3::new(0.1, 0.52, 0.0)),
        (&bones.foot_l, Some(12), Vec3::new(0.1, 0.08, 0.0)),
        (&bones.upper_leg_r, Some(0), Vec3::new(-0.1, 0.95, 0.0)),
        (&bones.lower_leg_r, Some(14), Vec3::new(-0.1, 0.52, 0.0)),
        (&bones.foot_r, Some(15), Vec3::new(-0.1, 0.08, 0.0)),
    ];
    let nodes = joints
        .iter()
        .map(|&(name, parent, position)| {
            let parent_position = parent.map_or(Vec3::ZERO, |p| joints[p].2);
            Node::new(
                name,
                parent,
                Transform::from_translation(position - parent_position),
            )
        })
        .collect();
    Hierarchy::new(nodes)
}

/// The bind pose of a hierarchy, for turning world points and directions into bone space.
struct Layout<'a> {
    /// The hierarchy the bones are looked up in.
    hierarchy: &'a Hierarchy,
    /// The bind pose of every bone in model space.
    bind: Vec<Pose>,
}

impl Layout<'_> {
    /// The index of the bone called `name`.
    fn index(&self, name: &str) -> Result<usize, RigError> {
        self.hierarchy
            .find(name)
            .ok_or_else(|| RigError::UnknownBone(name.to_owned()))
    }

    /// The model space bind position of the bone called `name`.
    fn position(&self, name: &str) -> Result<Vec3, RigError> {
        Ok(self.bind[self.index(name)?].position)
    }

    /// The bind pose of the bone called `name`.
    fn pose(&self, name: &str) -> Result<Pose, RigError> {
        Ok(self.bind[self.index(name)?])
    }

    /// A capsule between two model space points, as a shape of the bone called `bone`.
    fn capsule(&self, bone: &str, a: Vec3, b: Vec3, radius: f32) -> Result<RigShapeDef, RigError> {
        let pose = self.pose(bone)?;
        Ok(RigShapeDef::new(CapsuleCollider::new(
            pose.inv_transform_point(a),
            pose.inv_transform_point(b),
            radius,
        )))
    }

    /// A box with the model space axes centred on a model space point, as a shape of the bone
    /// called `bone`.
    fn cuboid(
        &self,
        bone: &str,
        center: Vec3,
        half_extents: Vec3,
    ) -> Result<RigShapeDef, RigError> {
        let pose = self.pose(bone)?;
        Ok(RigShapeDef::new(
            BoxCollider::new(half_extents)
                .with_center(pose.inv_transform_point(center))
                .with_rotation(pose.rotation.conjugate()),
        ))
    }

    /// A ball joint at the child bone with its twist axis along a model space direction.
    fn ball(
        &self,
        parent: &str,
        child: &str,
        axis: Vec3,
        limits: (f32, f32),
        scale: f32,
    ) -> Result<RigJoint, RigError> {
        let pose = self.pose(child)?;
        let kind = RigJointKind::Spherical {
            axis: pose.inv_rotate(axis),
            cone: Some(limits.0.to_radians()),
            twist: Some((-limits.1.to_radians(), limits.1.to_radians())),
        };
        Ok(Self::motorised(RigJoint::new(parent, child, kind), scale))
    }

    /// A sphere of `radius` at a model space point, as a shape of the bone called `bone`.
    fn ball_shape(&self, bone: &str, center: Vec3, radius: f32) -> Result<RigShapeDef, RigError> {
        let pose = self.pose(bone)?;
        Ok(RigShapeDef::new(
            SphereCollider::new(radius).with_center(pose.inv_transform_point(center)),
        ))
    }

    /// A hinge at the child bone about a model space direction, limited to `limits` degrees.
    fn hinge(
        &self,
        parent: &str,
        child: &str,
        axis: Vec3,
        limits: (f32, f32),
        scale: f32,
    ) -> Result<RigJoint, RigError> {
        let pose = self.pose(child)?;
        let kind = RigJointKind::Revolute {
            axis: pose.inv_rotate(axis),
            limits: Some((
                limits.0.to_radians().clamp(-PI, PI),
                limits.1.to_radians().clamp(-PI, PI),
            )),
        };
        Ok(Self::motorised(RigJoint::new(parent, child, kind), scale))
    }

    /// `joint` with the humanoid motor and friction scaled by `scale`.
    fn motorised(joint: RigJoint, scale: f32) -> RigJoint {
        joint
            .with_motor(JointMotor::new(
                MOTOR_STRENGTH,
                MOTOR_DAMPING,
                scale * MOTOR_TORQUE_LIMIT,
            ))
            .with_friction(scale * FRICTION_TORQUE)
    }
}

/// The definition of a humanoid rig on `hierarchy`, whose bones `bones` names: a body for every
/// bone, from pelvis to head and from upper arm to hand and from thigh to foot, with capsules,
/// a sphere and boxes of a 1.7 metre human; ball joints with cone and twist limits at the spine,
/// neck, head, shoulders, wrists and hips, hinges at the elbows and knees and ankle hinges that
/// let the feet pivot; every joint a weak spring towards the bind pose with a torque limit and a
/// little joint friction.
///
/// The bodies collide with each other like a person's limbs do: legs with legs, arms with the
/// torso, hands and feet with everything but the bone they hang from. Joined bodies and parts
/// that touch in the T-pose do not collide.
///
/// # Errors
///
/// Returns [`RigError::UnknownBone`] when the hierarchy lacks a bone `bones` names.
pub fn humanoid_rig(hierarchy: &Hierarchy, bones: &HumanoidBones) -> Result<RigDef, RigError> {
    let layout = Layout {
        hierarchy,
        bind: hierarchy
            .bind_world()
            .iter()
            .map(Pose::from_transform)
            .collect(),
    };
    let up = Vec3::Y;
    let spine = layout.position(&bones.spine)?;
    let chest = layout.position(&bones.chest)?;
    let neck = layout.position(&bones.neck)?;
    let head = layout.position(&bones.head)?;
    let mut def = RigDef::new()
        .with_body(RigBody::new(bones.hips.as_str()).with_shape(layout.capsule(
            &bones.hips,
            layout.position(&bones.upper_leg_l)?,
            layout.position(&bones.upper_leg_r)?,
            0.115,
        )?))
        .with_body(
            RigBody::new(bones.spine.as_str()).with_shape(layout.capsule(
                &bones.spine,
                spine,
                chest,
                0.1,
            )?),
        )
        .with_body(RigBody::new(bones.chest.as_str()).with_shape(layout.cuboid(
            &bones.chest,
            (chest + neck) * 0.5,
            Vec3::new(0.19, 0.5 * (neck.y - chest.y) + 0.02, 0.1),
        )?))
        .with_body(RigBody::new(bones.neck.as_str()).with_shape(layout.capsule(
            &bones.neck,
            neck,
            head,
            0.05,
        )?))
        .with_body(
            RigBody::new(bones.head.as_str()).with_shape(layout.ball_shape(
                &bones.head,
                head + up * 0.1,
                0.11,
            )?),
        )
        .with_joint(layout.ball(&bones.hips, &bones.spine, up, (35.0, 17.5), 1.0)?)
        .with_joint(layout.ball(&bones.spine, &bones.chest, up, (35.0, 17.5), 1.0)?)
        .with_joint(layout.ball(&bones.chest, &bones.neck, up, (30.0, 20.0), 0.8)?)
        .with_joint(layout.ball(&bones.neck, &bones.head, up, (40.0, 30.0), 0.6)?);
    let arms = [
        (
            &bones.upper_arm_l,
            &bones.lower_arm_l,
            &bones.hand_l,
            -Vec3::Y,
        ),
        (
            &bones.upper_arm_r,
            &bones.lower_arm_r,
            &bones.hand_r,
            Vec3::Y,
        ),
    ];
    for (upper, lower, hand, hinge) in arms {
        let shoulder = layout.position(upper)?;
        let elbow = layout.position(lower)?;
        let wrist = layout.position(hand)?;
        let upper_dir = (elbow - shoulder).normalize();
        let lower_dir = (wrist - elbow).normalize();
        def = def
            .with_body(
                RigBody::new(upper.as_str())
                    .with_shape(layout.capsule(upper, shoulder, elbow, 0.05)?),
            )
            .with_body(
                RigBody::new(lower.as_str())
                    .with_shape(layout.capsule(lower, elbow, wrist, 0.045)?),
            )
            .with_body(RigBody::new(hand.as_str()).with_shape(layout.capsule(
                hand,
                wrist,
                wrist + lower_dir * 0.14,
                0.04,
            )?))
            .with_joint(layout.ball(&bones.chest, upper, upper_dir, (80.0, 40.0), 0.8)?)
            .with_joint(layout.hinge(upper, lower, hinge, (0.0, 140.0), 0.06)?)
            .with_joint(layout.ball(lower, hand, lower_dir, (45.0, 20.0), 0.05)?);
    }
    let legs = [
        (&bones.upper_leg_l, &bones.lower_leg_l, &bones.foot_l),
        (&bones.upper_leg_r, &bones.lower_leg_r, &bones.foot_r),
    ];
    for (upper, lower, foot) in legs {
        let hip = layout.position(upper)?;
        let knee = layout.position(lower)?;
        let ankle = layout.position(foot)?;
        let upper_dir = (knee - hip).normalize();
        def = def
            .with_body(
                RigBody::new(upper.as_str()).with_shape(layout.capsule(upper, hip, knee, 0.075)?),
            )
            .with_body(
                RigBody::new(lower.as_str()).with_shape(layout.capsule(lower, knee, ankle, 0.06)?),
            )
            .with_body(
                RigBody::new(foot.as_str()).with_shape(
                    layout
                        .cuboid(
                            foot,
                            ankle + Vec3::new(0.0, -0.045, 0.06),
                            Vec3::new(0.05, 0.035, 0.11),
                        )?
                        .with_density(500.0),
                ),
            )
            .with_joint(layout.ball(&bones.hips, upper, upper_dir, (50.0, 20.0), 1.0)?)
            .with_joint(layout.hinge(upper, lower, Vec3::X, (0.0, 140.0), 0.5)?)
            .with_joint(layout.hinge(lower, foot, Vec3::X, (-20.0, 45.0), 0.3)?);
    }
    Ok(def)
}
