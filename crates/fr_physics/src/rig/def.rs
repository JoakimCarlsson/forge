//! The data a rig is made from: which bones get bodies and shapes, and how joints connect them.

use fr_math::Vec3;

use crate::geometry::Geometry;
use crate::joint::JointMotor;
use crate::shape::Material;

/// A reference to a bone of a hierarchy, by name or by index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoneRef {
    /// The bone with this name.
    Name(String),
    /// The bone at this index.
    Index(usize),
}

impl From<&str> for BoneRef {
    /// A reference by name.
    fn from(name: &str) -> Self {
        Self::Name(name.to_owned())
    }
}

impl From<String> for BoneRef {
    /// A reference by name.
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}

impl From<usize> for BoneRef {
    /// A reference by index.
    fn from(index: usize) -> Self {
        Self::Index(index)
    }
}

/// One collider of a rig body.
#[derive(Clone, Debug)]
pub struct RigShapeDef {
    /// The shape, in the frame of the bone; a collider type such as `SphereCollider`,
    /// `CapsuleCollider`, `BoxCollider` or `HullCollider` converts into it.
    pub geometry: Geometry,
    /// The mass per unit volume in kilograms per cubic metre.
    pub density: f32,
    /// The surface of the collider.
    pub material: Material,
}

impl RigShapeDef {
    /// A collider of `geometry` in the frame of the bone, at the density of water and with a
    /// slightly rolling-resistant material.
    pub fn new(geometry: impl Into<Geometry>) -> Self {
        Self {
            geometry: geometry.into(),
            density: 1000.0,
            material: Material {
                friction: 0.6,
                rolling_resistance: 0.05,
                ..Material::default()
            },
        }
    }

    /// The same collider with `density`.
    pub fn with_density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    /// The same collider with `material`.
    pub fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }
}

/// A rigid body that follows a bone, and with it every descendant bone that has no body of its
/// own.
#[derive(Clone, Debug)]
pub struct RigBody {
    /// The bone whose bind pose places the body.
    pub bone: BoneRef,
    /// The colliders of the body.
    pub shapes: Vec<RigShapeDef>,
}

impl RigBody {
    /// A body on `bone` with no shapes yet.
    pub fn new(bone: impl Into<BoneRef>) -> Self {
        Self {
            bone: bone.into(),
            shapes: Vec::new(),
        }
    }

    /// The same body with one more collider.
    pub fn with_shape(mut self, shape: RigShapeDef) -> Self {
        self.shapes.push(shape);
        self
    }
}

/// The type of a rig joint with the settings only that type has. Angles are in radians and axes
/// and anchors are in the bind frame of the child bone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RigJointKind {
    /// A ball and socket joint whose twist axis is `axis`.
    Spherical {
        /// The twist axis; the cone is measured from it.
        axis: Vec3,
        /// The largest swing away from the axis, if limited.
        cone: Option<f32>,
        /// The lowest and highest twist about the axis, if limited.
        twist: Option<(f32, f32)>,
    },
    /// A hinge about `axis`.
    Revolute {
        /// The hinge axis.
        axis: Vec3,
        /// The lowest and highest angle, if limited.
        limits: Option<(f32, f32)>,
    },
    /// Holds the two bones together; the motor strength is the softness of the weld, where zero
    /// is rigid.
    Weld,
    /// Keeps an anchor on the parent bone and the joint anchor on the child bone at a distance,
    /// as a spring whose strength is the motor strength.
    Distance {
        /// The anchor on the parent bone, in its bind frame.
        parent_anchor: Vec3,
        /// The rest length; the distance between the anchors in the bind pose when `None`.
        length: Option<f32>,
        /// The shortest and longest length, if limited.
        limits: Option<(f32, f32)>,
    },
}

/// One joint between two bones, driven by its motor towards a target pose.
#[derive(Clone, Debug, PartialEq)]
pub struct RigJoint {
    /// The bone on the parent side; its body is the body of the nearest bone at or above it
    /// that has one.
    pub parent: BoneRef,
    /// The bone on the child side, resolved to a body the same way.
    pub child: BoneRef,
    /// The type of the joint.
    pub kind: RigJointKind,
    /// Where the joint sits, in the bind frame of the child bone.
    pub anchor: Vec3,
    /// How hard the joint drives itself towards its target pose; the target is the bind pose
    /// until a pose is set on the rig.
    pub motor: JointMotor,
    /// The torque that resists motion of the joint, which is Box3D's joint friction.
    pub friction: f32,
    /// Whether the two bodies the joint connects collide with each other.
    pub collide_connected: bool,
}

impl RigJoint {
    /// A joint of `kind` from `parent` to `child` at the origin of the child bone, with a limp
    /// motor and no friction.
    pub fn new(parent: impl Into<BoneRef>, child: impl Into<BoneRef>, kind: RigJointKind) -> Self {
        Self {
            parent: parent.into(),
            child: child.into(),
            kind,
            anchor: Vec3::ZERO,
            motor: JointMotor::LIMP,
            friction: 0.0,
            collide_connected: false,
        }
    }

    /// The same joint at `anchor` in the bind frame of the child bone.
    pub fn with_anchor(mut self, anchor: Vec3) -> Self {
        self.anchor = anchor;
        self
    }

    /// The same joint with `motor`.
    pub fn with_motor(mut self, motor: JointMotor) -> Self {
        self.motor = motor;
        self
    }

    /// The same joint whose two bodies collide with each other.
    pub fn with_collide_connected(mut self) -> Self {
        self.collide_connected = true;
        self
    }

    /// The same joint with joint friction `torque`.
    pub fn with_friction(mut self, torque: f32) -> Self {
        self.friction = torque;
        self
    }
}

/// A rig described as data: bodies on bones and joints between them.
///
/// The definition says nothing about what the bones are; it is turned into bodies, shapes and
/// joints by [`crate::rig::Rig::build`] against a hierarchy that has the bones it names.
///
/// Bodies joined by a joint collide only when the joint says so. Every other pair of bodies
/// collides when `self_collision` is on, except the pairs listed in `ignored_pairs` and the
/// pairs whose shapes are closer than `rest_overlap_margin` in the bind pose, which would
/// otherwise push each other apart for ever.
#[derive(Clone, Debug)]
pub struct RigDef {
    /// The bodies, one per bone at most.
    pub bodies: Vec<RigBody>,
    /// The joints, in the order a rig numbers them.
    pub joints: Vec<RigJoint>,
    /// Whether the bodies of the rig collide with each other; when off no pair does.
    pub self_collision: bool,
    /// Pairs of bones whose bodies never collide with each other.
    pub ignored_pairs: Vec<(BoneRef, BoneRef)>,
    /// The surface distance in metres under which two bodies that no joint connects are
    /// treated as ignored pairs, measured in the bind pose.
    pub rest_overlap_margin: f32,
}

impl Default for RigDef {
    /// An empty definition whose bodies collide with each other except where they touch at rest,
    /// within one centimetre.
    fn default() -> Self {
        Self {
            bodies: Vec::new(),
            joints: Vec::new(),
            self_collision: true,
            ignored_pairs: Vec::new(),
            rest_overlap_margin: 0.01,
        }
    }
}

impl RigDef {
    /// An empty definition.
    pub fn new() -> Self {
        Self::default()
    }

    /// The same definition with one more body.
    pub fn with_body(mut self, body: RigBody) -> Self {
        self.bodies.push(body);
        self
    }

    /// The same definition with one more joint.
    pub fn with_joint(mut self, joint: RigJoint) -> Self {
        self.joints.push(joint);
        self
    }

    /// The same definition where the bodies of bones `a` and `b` never collide with each other.
    pub fn with_ignored_pair(mut self, a: impl Into<BoneRef>, b: impl Into<BoneRef>) -> Self {
        self.ignored_pairs.push((a.into(), b.into()));
        self
    }

    /// The same definition with self collision turned on or off.
    pub fn with_self_collision(mut self, on: bool) -> Self {
        self.self_collision = on;
        self
    }
}
