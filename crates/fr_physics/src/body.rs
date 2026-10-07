//! Rigid bodies: static, kinematic and dynamic, with their pose, velocity and mass.

use fr_math::{Mat3, Quat, Vec3};

use crate::constants::HUGE;
use crate::contact::ContactId;
use crate::geometry::MassData;
use crate::island::IslandId;
use crate::joint::JointId;
use crate::math::{Pose, invert_symmetric, steiner};
use crate::shape::ShapeId;
use crate::slot::Handle;

/// Marks [`BodyId`].
#[derive(Clone, Copy, Debug)]
pub struct BodyTag;

/// A handle to a body of a world.
pub type BodyId = Handle<BodyTag>;

/// How a body moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyType {
    /// Never moves and has infinite mass; collides with dynamic bodies only.
    Static,
    /// Moves by its velocity only and has infinite mass; collides with dynamic bodies only.
    Kinematic,
    /// Moves by forces, gravity and collisions.
    Dynamic,
}

/// The description of a body to create.
#[derive(Clone, Copy, Debug)]
pub struct BodyDef {
    /// How the body moves.
    pub body_type: BodyType,
    /// The position of the body origin.
    pub position: Vec3,
    /// The orientation of the body.
    pub rotation: Quat,
    /// The linear velocity of the body centre.
    pub linear_velocity: Vec3,
    /// The angular velocity in radians per second.
    pub angular_velocity: Vec3,
    /// The fraction of linear velocity lost per second.
    pub linear_damping: f32,
    /// The fraction of angular velocity lost per second.
    pub angular_damping: f32,
    /// How strongly gravity acts on the body.
    pub gravity_scale: f32,
    /// The speed below which the body counts as still, in metres per second.
    pub sleep_threshold: f32,
    /// How much of its size the body may move in a step before continuous collision is used.
    pub safety_factor: f32,
    /// Whether the body may fall asleep.
    pub enable_sleep: bool,
    /// Whether the body starts awake.
    pub is_awake: bool,
    /// A value the game attaches to the body.
    pub user_data: u64,
}

impl BodyDef {
    /// A static body at the origin.
    pub fn new(body_type: BodyType) -> Self {
        Self {
            body_type,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            linear_damping: 0.0,
            angular_damping: 0.0,
            gravity_scale: 1.0,
            sleep_threshold: 0.05,
            safety_factor: 0.5,
            enable_sleep: true,
            is_awake: true,
            user_data: 0,
        }
    }

    /// A dynamic body at `position`.
    pub fn dynamic(position: Vec3) -> Self {
        Self {
            position,
            ..Self::new(BodyType::Dynamic)
        }
    }

    /// The same body at `position`.
    pub fn at(mut self, position: Vec3) -> Self {
        self.position = position;
        self
    }

    /// The same body with `rotation`.
    pub fn rotated(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }
}

/// A body of a world.
#[derive(Clone, Debug)]
pub(crate) struct Body {
    /// How the body moves.
    pub(crate) body_type: BodyType,
    /// The pose of the body origin.
    pub(crate) pose: Pose,
    /// The pose of the body origin at the start of the last step.
    pub(crate) previous_pose: Pose,
    /// The centre of mass in the world.
    pub(crate) center: Vec3,
    /// The centre of mass at the start of the step, the start of a continuous sweep.
    pub(crate) center0: Vec3,
    /// The orientation at the start of the step.
    pub(crate) rotation0: Quat,
    /// The centre of mass in the frame of the body.
    pub(crate) local_center: Vec3,
    /// The linear velocity of the centre of mass.
    pub(crate) linear_velocity: Vec3,
    /// The angular velocity.
    pub(crate) angular_velocity: Vec3,
    /// The force applied during the next step.
    pub(crate) force: Vec3,
    /// The torque applied during the next step.
    pub(crate) torque: Vec3,
    /// The mass.
    pub(crate) mass: f32,
    /// The inverse mass, zero for a body that is not dynamic.
    pub(crate) inv_mass: f32,
    /// The inertia tensor about the centre of mass in the frame of the body.
    pub(crate) inertia_local: Mat3,
    /// The inverse inertia tensor in the frame of the body.
    pub(crate) inv_inertia_local: Mat3,
    /// The inverse inertia tensor in the world frame.
    pub(crate) inv_inertia_world: Mat3,
    /// The radius of the largest sphere about the centre inside the body.
    pub(crate) min_extent: f32,
    /// The farthest reach of the body from the centre along each axis.
    pub(crate) max_extent: Vec3,
    /// The fraction of linear velocity lost per second.
    pub(crate) linear_damping: f32,
    /// The fraction of angular velocity lost per second.
    pub(crate) angular_damping: f32,
    /// How strongly gravity acts.
    pub(crate) gravity_scale: f32,
    /// The speed below which the body is still.
    pub(crate) sleep_threshold: f32,
    /// How long the body has been still.
    pub(crate) sleep_time: f32,
    /// The fraction of its size the body may move before continuous collision.
    pub(crate) safety_factor: f32,
    /// Whether the body may fall asleep.
    pub(crate) enable_sleep: bool,
    /// Whether the body is asleep.
    pub(crate) asleep: bool,
    /// Whether the body moved far enough this step to need continuous collision.
    pub(crate) is_fast: bool,
    /// The speed the body had at the end of the last step, for sleeping.
    pub(crate) sleep_velocity: f32,
    /// The colliders of the body in creation order.
    pub(crate) shapes: Vec<ShapeId>,
    /// The joints of the body in creation order.
    pub(crate) joints: Vec<JointId>,
    /// The bodies this body never collides with, in the order they were added.
    pub(crate) ignored: Vec<BodyId>,
    /// The contacts of the body, kept only for bodies that are not static.
    pub(crate) contacts: Vec<ContactId>,
    /// The island the body belongs to, if it is not static.
    pub(crate) island: IslandId,
    /// The position of the body in its island.
    pub(crate) island_index: usize,
    /// The index of the body in the solver arrays during a step.
    pub(crate) solver_index: usize,
    /// The game's value.
    pub(crate) user_data: u64,
}

impl Body {
    /// A body from its description.
    pub(crate) fn new(def: &BodyDef) -> Self {
        let pose = Pose::new(def.position, def.rotation);
        let moves = def.body_type != BodyType::Static;
        Self {
            body_type: def.body_type,
            pose,
            previous_pose: pose,
            center: def.position,
            center0: def.position,
            rotation0: def.rotation,
            local_center: Vec3::ZERO,
            linear_velocity: if moves {
                def.linear_velocity
            } else {
                Vec3::ZERO
            },
            angular_velocity: if moves {
                def.angular_velocity
            } else {
                Vec3::ZERO
            },
            force: Vec3::ZERO,
            torque: Vec3::ZERO,
            mass: 0.0,
            inv_mass: 0.0,
            inertia_local: Mat3::ZERO,
            inv_inertia_local: Mat3::ZERO,
            inv_inertia_world: Mat3::ZERO,
            min_extent: HUGE,
            max_extent: Vec3::ZERO,
            linear_damping: def.linear_damping,
            angular_damping: def.angular_damping,
            gravity_scale: def.gravity_scale,
            sleep_threshold: def.sleep_threshold,
            sleep_time: 0.0,
            safety_factor: def.safety_factor,
            enable_sleep: def.enable_sleep,
            asleep: !def.is_awake && moves,
            is_fast: false,
            sleep_velocity: 0.0,
            shapes: Vec::new(),
            joints: Vec::new(),
            ignored: Vec::new(),
            contacts: Vec::new(),
            island: IslandId::NULL,
            island_index: usize::MAX,
            solver_index: usize::MAX,
            user_data: def.user_data,
        }
    }

    /// Whether the body takes part in the current step: not static and not asleep.
    pub(crate) fn is_awake(&self) -> bool {
        self.body_type != BodyType::Static && !self.asleep
    }

    /// Whether the body is dynamic.
    pub(crate) fn is_dynamic(&self) -> bool {
        self.body_type == BodyType::Dynamic
    }

    /// Recomputes the world inverse inertia from the current orientation.
    pub(crate) fn update_inverse_inertia_world(&mut self) {
        let rotation = Mat3::from_quat(self.pose.rotation);
        self.inv_inertia_world = (rotation * self.inv_inertia_local) * rotation.transpose();
    }

    /// Sets the mass properties from the mass data of the colliders with density and the
    /// geometries of all colliders.
    pub(crate) fn apply_mass(
        &mut self,
        masses: &[MassData],
        geometries: &[&crate::geometry::Geometry],
    ) {
        self.mass = 0.0;
        self.inertia_local = Mat3::ZERO;
        self.inv_mass = 0.0;
        self.inv_inertia_local = Mat3::ZERO;
        self.inv_inertia_world = Mat3::ZERO;
        self.local_center = Vec3::ZERO;
        self.min_extent = HUGE;
        self.max_extent = Vec3::ZERO;
        if self.body_type != BodyType::Dynamic {
            self.center = self.pose.position;
            self.center0 = self.center;
            if self.body_type == BodyType::Kinematic {
                self.fold_extents(geometries);
            }
            return;
        }
        let mut local_center = Vec3::ZERO;
        for data in masses {
            self.mass += data.mass;
            local_center += data.center * data.mass;
        }
        if self.mass > 0.0 {
            self.inv_mass = 1.0 / self.mass;
            local_center *= self.inv_mass;
        }
        for data in masses {
            if data.mass == 0.0 {
                continue;
            }
            let offset = local_center - data.center;
            self.inertia_local += data.inertia + steiner(data.mass, offset);
        }
        if crate::math::det(self.inertia_local) > 0.0 {
            self.inv_inertia_local = invert_symmetric(self.inertia_local);
            self.update_inverse_inertia_world();
        }
        let old_center = self.center;
        self.local_center = local_center;
        self.center = self.pose.transform_point(local_center);
        self.center0 = self.center;
        self.linear_velocity += self.angular_velocity.cross(self.center - old_center);
        self.fold_extents(geometries);
    }

    /// Folds the extents of the colliders about the centre into the body extents.
    fn fold_extents(&mut self, geometries: &[&crate::geometry::Geometry]) {
        for geometry in geometries {
            let extent = geometry.extent(self.local_center);
            self.min_extent = self.min_extent.min(extent.min);
            self.max_extent = self.max_extent.max(extent.max.abs());
        }
    }
}
