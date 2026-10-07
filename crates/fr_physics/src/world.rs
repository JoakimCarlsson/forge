//! The physics world: bodies, colliders and joints, with the creation, destruction and access
//! API. Stepping is in [`step`](crate::step).

use std::collections::HashSet;

use fr_math::{Quat, Vec3};

use crate::body::{Body, BodyDef, BodyId, BodyTag, BodyType};
use crate::broad_phase::BroadPhase;
use crate::constants::{CONTACT_RECYCLE_DISTANCE, SPECULATIVE_DISTANCE};
use crate::contact::{Contact, ContactId, ContactTag, geometry_rank};
use crate::geometry::{Geometry, MassData};
use crate::island::{Island, IslandId, IslandTag};
use crate::joint::{Joint, JointDef, JointId, JointKind, JointTag, MouseJoint};
use crate::math::Pose;
use crate::shape::{Filter, Material, Shape, ShapeDef, ShapeId, ShapeTag};
use crate::slot::SlotMap;
use crate::solver::StepContext;
use fr_math::Aabb;

/// The settings of a world.
#[derive(Clone, Copy, Debug)]
pub struct WorldDef {
    /// The acceleration of gravity in metres per second squared.
    pub gravity: Vec3,
    /// The approach speed below which restitution is ignored.
    pub restitution_threshold: f32,
    /// The number of restitution passes at the end of a step.
    pub restitution_iterations: u32,
    /// The largest speed contacts push overlapping bodies apart at.
    pub contact_speed: f32,
    /// The stiffness of contacts in hertz.
    pub contact_hertz: f32,
    /// The damping ratio of contacts.
    pub contact_damping_ratio: f32,
    /// The largest linear speed of a body.
    pub maximum_linear_speed: f32,
    /// Whether islands fall asleep.
    pub enable_sleep: bool,
    /// Whether fast bodies are swept against static shapes.
    pub enable_continuous: bool,
    /// Whether impulses of the previous step are applied at the start of a step.
    pub enable_warm_starting: bool,
    /// The distance a contact may be reused without a new manifold; zero disables recycling.
    pub contact_recycle_distance: f32,
}

impl Default for WorldDef {
    /// Gravity of ten metres per second squared downwards and the Box3D tuning.
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -10.0, 0.0),
            restitution_threshold: 1.0,
            restitution_iterations: 2,
            contact_speed: 3.0,
            contact_hertz: 30.0,
            contact_damping_ratio: 10.0,
            maximum_linear_speed: 400.0,
            enable_sleep: true,
            enable_continuous: true,
            enable_warm_starting: true,
            contact_recycle_distance: CONTACT_RECYCLE_DISTANCE,
        }
    }
}

/// A pair of shapes that began or stopped touching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactEvent {
    /// The first shape of the contact.
    pub shape_a: ShapeId,
    /// The second shape of the contact.
    pub shape_b: ShapeId,
}

/// A shape that began or stopped overlapping a sensor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensorEvent {
    /// The sensor shape.
    pub sensor: ShapeId,
    /// The shape that overlaps it.
    pub visitor: ShapeId,
}

/// The counts of a world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// The number of bodies of all types.
    pub bodies: usize,
    /// The number of dynamic and kinematic bodies that are awake.
    pub awake_bodies: usize,
    /// The number of dynamic and kinematic bodies that sleep.
    pub sleeping_bodies: usize,
    /// The number of shapes.
    pub shapes: usize,
    /// The number of joints.
    pub joints: usize,
    /// The number of contacts including those that do not touch.
    pub contacts: usize,
    /// The number of contacts that touch.
    pub touching_contacts: usize,
    /// The number of islands.
    pub islands: usize,
    /// The number of steps taken.
    pub steps: u64,
}

/// A rigid body simulation.
pub struct World {
    /// The settings.
    pub(crate) def: WorldDef,
    /// The bodies.
    pub(crate) bodies: SlotMap<Body, BodyTag>,
    /// The colliders.
    pub(crate) shapes: SlotMap<Shape, ShapeTag>,
    /// The joints.
    pub(crate) joints: SlotMap<Joint, JointTag>,
    /// The contacts.
    pub(crate) contacts: SlotMap<Contact, ContactTag>,
    /// The islands.
    pub(crate) islands: SlotMap<Island, IslandTag>,
    /// The broad phase.
    pub(crate) broad_phase: BroadPhase,
    /// The pairs of shape slots that have a contact.
    pub(crate) pair_set: HashSet<u64>,
    /// The island that wants to sleep but must be split first.
    pub(crate) split_island: Option<IslandId>,
    /// The contacts that began touching during the last step.
    pub(crate) contact_begin_events: Vec<ContactEvent>,
    /// The contacts that stopped touching during the last step.
    pub(crate) contact_end_events: Vec<ContactEvent>,
    /// The sensor overlaps that began during the last step.
    pub(crate) sensor_begin_events: Vec<SensorEvent>,
    /// The sensor overlaps that ended during the last step.
    pub(crate) sensor_end_events: Vec<SensorEvent>,
    /// The number of steps taken.
    pub(crate) step_count: u64,
    /// The context of the last step, kept to read reaction forces.
    pub(crate) last_context: Option<StepContext>,
}

impl World {
    /// An empty world.
    pub fn new(def: WorldDef) -> Self {
        Self {
            def,
            bodies: SlotMap::default(),
            shapes: SlotMap::default(),
            joints: SlotMap::default(),
            contacts: SlotMap::default(),
            islands: SlotMap::default(),
            broad_phase: BroadPhase::default(),
            pair_set: HashSet::new(),
            split_island: None,
            contact_begin_events: Vec::new(),
            contact_end_events: Vec::new(),
            sensor_begin_events: Vec::new(),
            sensor_end_events: Vec::new(),
            step_count: 0,
            last_context: None,
        }
    }

    /// The settings.
    pub fn def(&self) -> &WorldDef {
        &self.def
    }

    /// Changes the gravity.
    pub fn set_gravity(&mut self, gravity: Vec3) {
        self.def.gravity = gravity;
    }

    /// Turns sleeping on or off; turning it off wakes every body.
    pub fn enable_sleeping(&mut self, enable: bool) {
        self.def.enable_sleep = enable;
        if !enable {
            let islands: Vec<IslandId> = self.islands.handles().collect();
            for island in islands {
                self.wake_island(island);
            }
        }
    }

    /// The counts of bodies, shapes, joints, contacts and islands.
    pub fn stats(&self) -> Stats {
        let mut stats = Stats {
            bodies: self.bodies.len(),
            shapes: self.shapes.len(),
            joints: self.joints.len(),
            contacts: self.contacts.len(),
            islands: self.islands.len(),
            steps: self.step_count,
            ..Stats::default()
        };
        for (_, body) in self.bodies.iter() {
            if body.body_type != BodyType::Static {
                if body.asleep {
                    stats.sleeping_bodies += 1;
                } else {
                    stats.awake_bodies += 1;
                }
            }
        }
        stats.touching_contacts = self.contacts.iter().filter(|(_, c)| c.touching).count();
        stats
    }

    /// The contacts that began touching during the last step.
    pub fn contact_begin_events(&self) -> &[ContactEvent] {
        &self.contact_begin_events
    }

    /// The contacts that stopped touching during the last step.
    pub fn contact_end_events(&self) -> &[ContactEvent] {
        &self.contact_end_events
    }

    /// The sensor overlaps that began during the last step.
    pub fn sensor_begin_events(&self) -> &[SensorEvent] {
        &self.sensor_begin_events
    }

    /// The sensor overlaps that ended during the last step.
    pub fn sensor_end_events(&self) -> &[SensorEvent] {
        &self.sensor_end_events
    }

    /// Creates a body.
    pub fn create_body(&mut self, def: &BodyDef) -> BodyId {
        let body = Body::new(def);
        let asleep = body.asleep;
        let moves = body.body_type != BodyType::Static;
        let id = self.bodies.insert(body);
        if moves {
            self.create_island_for_body(id, asleep);
        }
        id
    }

    /// Destroys a body with its joints, shapes and contacts.
    pub fn destroy_body(&mut self, id: BodyId) {
        let Some(body) = self.bodies.get(id) else {
            return;
        };
        let joints = body.joints.clone();
        let shapes = body.shapes.clone();
        let ignored = body.ignored.clone();
        for other in ignored {
            if let Some(other) = self.bodies.get_mut(other) {
                other.ignored.retain(|&candidate| candidate != id);
            }
        }
        for joint in joints {
            self.destroy_joint(joint);
        }
        for shape in shapes {
            self.destroy_shape_internal(shape, false);
        }
        self.remove_body_from_island(id);
        self.bodies.remove(id);
    }

    /// Whether a handle names a live body.
    pub fn contains_body(&self, id: BodyId) -> bool {
        self.bodies.contains(id)
    }

    /// The live bodies in creation slot order.
    pub fn bodies(&self) -> impl Iterator<Item = BodyId> + '_ {
        self.bodies.handles()
    }

    /// The pose of the origin of a body.
    pub fn body_pose(&self, id: BodyId) -> Option<Pose> {
        self.bodies.get(id).map(|b| b.pose)
    }

    /// The pose of the origin of a body at the start of the last step.
    pub fn body_previous_pose(&self, id: BodyId) -> Option<Pose> {
        self.bodies.get(id).map(|b| b.previous_pose)
    }

    /// The pose of a body blended between the start and the end of the last step, for drawing
    /// between fixed steps; `alpha` is zero at the start and one at the end.
    pub fn body_interpolated_pose(&self, id: BodyId, alpha: f32) -> Option<Pose> {
        let body = self.bodies.get(id)?;
        let a = body.previous_pose;
        let b = body.pose;
        Some(Pose::new(
            a.position + (b.position - a.position) * alpha,
            a.rotation.slerp(b.rotation, alpha),
        ))
    }

    /// The type of a body.
    pub fn body_type(&self, id: BodyId) -> Option<BodyType> {
        self.bodies.get(id).map(|b| b.body_type)
    }

    /// The linear velocity of the centre of mass of a body.
    pub fn body_linear_velocity(&self, id: BodyId) -> Option<Vec3> {
        self.bodies.get(id).map(|b| b.linear_velocity)
    }

    /// The angular velocity of a body.
    pub fn body_angular_velocity(&self, id: BodyId) -> Option<Vec3> {
        self.bodies.get(id).map(|b| b.angular_velocity)
    }

    /// The mass of a body.
    pub fn body_mass(&self, id: BodyId) -> Option<f32> {
        self.bodies.get(id).map(|b| b.mass)
    }

    /// The centre of mass of a body in the world.
    pub fn body_center_of_mass(&self, id: BodyId) -> Option<Vec3> {
        self.bodies.get(id).map(|b| b.center)
    }

    /// Whether a body is awake; a static body never is.
    pub fn is_body_awake(&self, id: BodyId) -> bool {
        self.bodies.get(id).is_some_and(Body::is_awake)
    }

    /// The colliders of a body.
    pub fn body_shapes(&self, id: BodyId) -> Vec<ShapeId> {
        self.bodies
            .get(id)
            .map_or_else(Vec::new, |b| b.shapes.clone())
    }

    /// The joints of a body.
    pub fn body_joints(&self, id: BodyId) -> Vec<JointId> {
        self.bodies
            .get(id)
            .map_or_else(Vec::new, |b| b.joints.clone())
    }

    /// The value the game attached to a body.
    pub fn body_user_data(&self, id: BodyId) -> Option<u64> {
        self.bodies.get(id).map(|b| b.user_data)
    }

    /// Wakes a body and the bodies it is linked to.
    pub fn wake_body(&mut self, id: BodyId) {
        if let Some(body) = self.bodies.get(id) {
            let island = body.island;
            if body.asleep {
                self.wake_island(island);
            }
        }
    }

    /// Sets the linear velocity of a body and wakes it.
    pub fn set_body_linear_velocity(&mut self, id: BodyId, velocity: Vec3) {
        self.wake_body(id);
        if let Some(body) = self.bodies.get_mut(id)
            && body.body_type != BodyType::Static
        {
            body.linear_velocity = velocity;
        }
    }

    /// Sets the angular velocity of a body and wakes it.
    pub fn set_body_angular_velocity(&mut self, id: BodyId, velocity: Vec3) {
        self.wake_body(id);
        if let Some(body) = self.bodies.get_mut(id)
            && body.body_type != BodyType::Static
        {
            body.angular_velocity = velocity;
        }
    }

    /// Applies a force at a world point for the next step.
    pub fn apply_force(&mut self, id: BodyId, force: Vec3, point: Vec3) {
        self.wake_body(id);
        if let Some(body) = self.bodies.get_mut(id)
            && body.is_dynamic()
        {
            body.force += force;
            body.torque += (point - body.center).cross(force);
        }
    }

    /// Applies a force through the centre of mass for the next step.
    pub fn apply_force_to_center(&mut self, id: BodyId, force: Vec3) {
        self.wake_body(id);
        if let Some(body) = self.bodies.get_mut(id)
            && body.is_dynamic()
        {
            body.force += force;
        }
    }

    /// Applies a torque for the next step.
    pub fn apply_torque(&mut self, id: BodyId, torque: Vec3) {
        self.wake_body(id);
        if let Some(body) = self.bodies.get_mut(id)
            && body.is_dynamic()
        {
            body.torque += torque;
        }
    }

    /// Applies an impulse at a world point, changing the velocity at once.
    pub fn apply_linear_impulse(&mut self, id: BodyId, impulse: Vec3, point: Vec3) {
        self.wake_body(id);
        if let Some(body) = self.bodies.get_mut(id)
            && body.is_dynamic()
        {
            body.linear_velocity += impulse * body.inv_mass;
            body.angular_velocity += body.inv_inertia_world * (point - body.center).cross(impulse);
        }
    }

    /// Applies an angular impulse, changing the velocity at once.
    pub fn apply_angular_impulse(&mut self, id: BodyId, impulse: Vec3) {
        self.wake_body(id);
        if let Some(body) = self.bodies.get_mut(id)
            && body.is_dynamic()
        {
            body.angular_velocity += body.inv_inertia_world * impulse;
        }
    }

    /// Teleports a body to a pose, updating its colliders and waking it.
    pub fn set_body_pose(&mut self, id: BodyId, pose: Pose) {
        self.wake_body(id);
        let Some(body) = self.bodies.get_mut(id) else {
            return;
        };
        body.pose = pose;
        body.previous_pose = pose;
        body.center = pose.transform_point(body.local_center);
        body.center0 = body.center;
        body.rotation0 = pose.rotation;
        body.update_inverse_inertia_world();
        let shapes = body.shapes.clone();
        let contacts = body.contacts.clone();
        for contact in contacts {
            if let Some(contact) = self.contacts.get_mut(contact) {
                contact.relative_pose_valid = false;
            }
        }
        for shape in shapes {
            self.refresh_shape_bounds(shape);
        }
    }

    /// Attaches a collider to a body. Returns `None` for a stale body handle.
    pub fn create_shape(&mut self, body: BodyId, def: &ShapeDef) -> Option<ShapeId> {
        let body_ref = self.bodies.get(body)?;
        let body_type = body_ref.body_type;
        let aabb = def.geometry.compute_aabb(&body_ref.pose);
        let margin = Shape::margin_of(&def.geometry);
        let tight = aabb.inflate(SPECULATIVE_DISTANCE);
        let fat_margin = if body_type == BodyType::Static {
            SPECULATIVE_DISTANCE
        } else {
            margin
        };
        let shape = Shape {
            body,
            geometry: def.geometry.clone(),
            density: def.density,
            material: def.material,
            filter: def.filter,
            is_sensor: def.is_sensor,
            enable_sensor_events: def.enable_sensor_events,
            enable_contact_events: def.enable_contact_events,
            user_data: def.user_data,
            aabb: tight,
            fat_aabb: tight.inflate(fat_margin),
            aabb_margin: margin,
            proxy: crate::tree::NULL_NODE,
            moved: true,
            sensor_overlaps: Vec::new(),
        };
        let fat = shape.fat_aabb;
        let id = self.shapes.insert(shape);
        let proxy = self
            .broad_phase
            .create_proxy(body_type, &fat, id.index() as u32);
        if let Some(shape) = self.shapes.get_mut(id) {
            shape.proxy = proxy;
        }
        self.broad_phase.mark_moved(id);
        if let Some(body) = self.bodies.get_mut(body) {
            body.shapes.push(id);
        }
        if def.update_body_mass {
            self.update_body_mass(body);
        }
        self.wake_body(body);
        Some(id)
    }

    /// Detaches and destroys a collider with its contacts.
    pub fn destroy_shape(&mut self, id: ShapeId) {
        self.destroy_shape_internal(id, true);
    }

    /// Destroys a collider, updating the mass of its body when `update_mass`.
    fn destroy_shape_internal(&mut self, id: ShapeId, update_mass: bool) {
        let Some(shape) = self.shapes.get(id) else {
            return;
        };
        let body_id = shape.body;
        let proxy = shape.proxy;
        let contacts: Vec<ContactId> = self
            .contacts
            .iter()
            .filter(|(_, c)| c.shape_a == id || c.shape_b == id)
            .map(|(contact_id, _)| contact_id)
            .collect();
        for contact in contacts {
            self.destroy_contact(contact, true);
        }
        let body_type = self.bodies.get(body_id).map(|b| b.body_type);
        if let Some(body_type) = body_type {
            self.broad_phase.destroy_proxy(body_type, proxy);
        }
        if let Some(body) = self.bodies.get_mut(body_id) {
            body.shapes.retain(|&s| s != id);
        }
        self.shapes.remove(id);
        if update_mass {
            self.update_body_mass(body_id);
        }
    }

    /// The body a collider is attached to.
    pub fn shape_body(&self, id: ShapeId) -> Option<BodyId> {
        self.shapes.get(id).map(|s| s.body)
    }

    /// The geometry of a collider in the frame of its body.
    pub fn shape_geometry(&self, id: ShapeId) -> Option<&Geometry> {
        self.shapes.get(id).map(|s| &s.geometry)
    }

    /// The material of a collider.
    pub fn shape_material(&self, id: ShapeId) -> Option<Material> {
        self.shapes.get(id).map(|s| s.material)
    }

    /// Replaces the material of a collider.
    pub fn set_shape_material(&mut self, id: ShapeId, material: Material) {
        if let Some(shape) = self.shapes.get_mut(id) {
            shape.material = material;
        }
    }

    /// The collision filter of a collider.
    pub fn shape_filter(&self, id: ShapeId) -> Option<Filter> {
        self.shapes.get(id).map(|s| s.filter)
    }

    /// Whether a collider is a sensor.
    pub fn shape_is_sensor(&self, id: ShapeId) -> bool {
        self.shapes.get(id).is_some_and(|s| s.is_sensor)
    }

    /// The value the game attached to a collider.
    pub fn shape_user_data(&self, id: ShapeId) -> Option<u64> {
        self.shapes.get(id).map(|s| s.user_data)
    }

    /// The world bounds of a collider.
    pub fn shape_aabb(&self, id: ShapeId) -> Option<Aabb> {
        self.shapes.get(id).map(|s| s.aabb)
    }

    /// The shapes a sensor overlapped at the end of the last step, in handle order.
    pub fn sensor_overlaps(&self, id: ShapeId) -> &[ShapeId] {
        self.shapes
            .get(id)
            .map_or(&[], |s| s.sensor_overlaps.as_slice())
    }

    /// Recomputes the mass properties of a body from its colliders.
    pub(crate) fn update_body_mass(&mut self, body_id: BodyId) {
        let Some(body) = self.bodies.get(body_id) else {
            return;
        };
        let mut masses: Vec<MassData> = Vec::new();
        let mut geometries: Vec<Geometry> = Vec::new();
        for &shape_id in &body.shapes {
            if let Some(shape) = self.shapes.get(shape_id) {
                if shape.is_sensor {
                    continue;
                }
                geometries.push(shape.geometry.clone());
                if shape.density > 0.0 {
                    masses.push(shape.geometry.compute_mass(shape.density));
                }
            }
        }
        let contacts = body.contacts.clone();
        let refs: Vec<&Geometry> = geometries.iter().collect();
        if let Some(body) = self.bodies.get_mut(body_id) {
            body.apply_mass(&masses, &refs);
        }
        for contact in contacts {
            if let Some(contact) = self.contacts.get_mut(contact) {
                contact.relative_pose_valid = false;
            }
        }
    }

    /// Recomputes the bounds of a collider from the pose of its body and refreshes its proxy.
    pub(crate) fn refresh_shape_bounds(&mut self, id: ShapeId) {
        let Some(shape) = self.shapes.get(id) else {
            return;
        };
        let Some(body) = self.bodies.get(shape.body) else {
            return;
        };
        let body_type = body.body_type;
        let aabb = shape
            .geometry
            .compute_aabb(&body.pose)
            .inflate(SPECULATIVE_DISTANCE);
        let margin = if body_type == BodyType::Static {
            SPECULATIVE_DISTANCE
        } else {
            shape.aabb_margin
        };
        let proxy = shape.proxy;
        let contained = shape.fat_aabb.contains(&aabb);
        let fat = aabb.inflate(margin);
        let moved = shape.moved;
        if let Some(shape) = self.shapes.get_mut(id) {
            shape.aabb = aabb;
            if !contained {
                shape.fat_aabb = fat;
                shape.moved = true;
            }
        }
        if !contained {
            self.broad_phase.move_proxy(body_type, proxy, &fat);
            if !moved {
                self.broad_phase.mark_moved(id);
            }
        }
    }

    /// Creates a joint. Returns `None` when a body is stale or both are the same body.
    pub fn create_joint(&mut self, def: JointDef) -> Option<JointId> {
        if def.body_a == def.body_b {
            return None;
        }
        let body_a = self.bodies.get(def.body_a)?;
        let body_b = self.bodies.get(def.body_b)?;
        let linked = Joint::has_dynamic_body(body_a, body_b);
        let (a, b, collide) = (def.body_a, def.body_b, def.collide_connected);
        let id = self.joints.insert(Joint::new(def));
        for body in [a, b] {
            if let Some(body) = self.bodies.get_mut(body) {
                body.joints.push(id);
            }
        }
        if !collide {
            self.destroy_contacts_between(a, b);
        }
        if linked {
            self.link_joint(id);
        }
        Some(id)
    }

    /// Destroys a joint.
    pub fn destroy_joint(&mut self, id: JointId) {
        let Some(joint) = self.joints.get(id) else {
            return;
        };
        let (a, b) = (joint.body_a, joint.body_b);
        self.unlink_joint(id);
        self.joints.remove(id);
        for body in [a, b] {
            if let Some(body) = self.bodies.get_mut(body) {
                body.joints.retain(|&j| j != id);
            }
        }
        for body in [a, b] {
            self.wake_body(body);
        }
    }

    /// The type and settings of a joint.
    pub fn joint(&self, id: JointId) -> Option<&JointKind> {
        self.joints.get(id).map(|j| &j.kind)
    }

    /// The type and settings of a joint, for changing springs, limits and motors.
    pub fn joint_mut(&mut self, id: JointId) -> Option<&mut JointKind> {
        self.joints.get_mut(id).map(|j| &mut j.kind)
    }

    /// The two bodies of a joint.
    pub fn joint_bodies(&self, id: JointId) -> Option<(BodyId, BodyId)> {
        self.joints.get(id).map(|j| (j.body_a, j.body_b))
    }

    /// Grabs `body` at the world point `anchor` with a mouse joint that pulls it towards the
    /// anchor, holding it against `ground`, a static body that is the other end of the joint.
    /// The bodies keep colliding. Returns `None` when a body is stale or the same.
    pub fn create_mouse_joint(
        &mut self,
        ground: BodyId,
        body: BodyId,
        anchor: Vec3,
        mut joint: MouseJoint,
    ) -> Option<JointId> {
        let pose = self.body_pose(body)?;
        joint.target = anchor;
        let mut def = JointDef::new(
            ground,
            body,
            Pose::IDENTITY,
            Pose::from_position(pose.inv_transform_point(anchor)),
            JointKind::Mouse(joint),
        );
        def.collide_connected = true;
        self.create_joint(def)
    }

    /// Moves the target of a mouse joint and wakes the body it holds.
    pub fn set_mouse_joint_target(&mut self, id: JointId, target: Vec3) {
        let Some(joint) = self.joints.get_mut(id) else {
            return;
        };
        let body = joint.body_b;
        if let JointKind::Mouse(mouse) = &mut joint.kind {
            mouse.target = target;
            self.wake_body(body);
        }
    }

    /// Makes two bodies never collide with each other, whatever their shapes' filters and
    /// joints say, and destroys the contacts between them. Does nothing for the same body or a
    /// stale handle.
    pub fn disable_collision_between(&mut self, a: BodyId, b: BodyId) {
        if a == b || !self.bodies.contains(a) || !self.bodies.contains(b) {
            return;
        }
        for (body, other) in [(a, b), (b, a)] {
            if let Some(body) = self.bodies.get_mut(body)
                && !body.ignored.contains(&other)
            {
                body.ignored.push(other);
            }
        }
        self.destroy_contacts_between(a, b);
    }

    /// Whether two bodies were made never to collide by [`World::disable_collision_between`].
    pub fn is_collision_disabled(&self, a: BodyId, b: BodyId) -> bool {
        self.bodies
            .get(a)
            .is_some_and(|body| body.ignored.contains(&b))
    }

    /// Destroys the contacts between two bodies.
    fn destroy_contacts_between(&mut self, a: BodyId, b: BodyId) {
        let source = match self.bodies.get(a) {
            Some(body) if body.body_type != BodyType::Static => a,
            _ => b,
        };
        let other = if source == a { b } else { a };
        let contacts: Vec<ContactId> = self
            .bodies
            .get(source)
            .map(|body| body.contacts.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|&id| {
                self.contacts
                    .get(id)
                    .is_some_and(|c| c.body_a == other || c.body_b == other)
            })
            .collect();
        for id in contacts {
            self.destroy_contact(id, false);
        }
    }

    /// Whether two bodies may collide: at least one is dynamic, they were not made to ignore each other and no joint between them
    /// disables collision.
    pub(crate) fn should_bodies_collide(&self, a: BodyId, b: BodyId) -> bool {
        let (Some(body_a), Some(body_b)) = (self.bodies.get(a), self.bodies.get(b)) else {
            return false;
        };
        if !body_a.is_dynamic() && !body_b.is_dynamic() {
            return false;
        }
        if body_a.ignored.contains(&b) {
            return false;
        }
        let (source, other) = if body_a.joints.len() < body_b.joints.len() {
            (body_a, b)
        } else {
            (body_b, a)
        };
        for &joint_id in &source.joints {
            if let Some(joint) = self.joints.get(joint_id) {
                let connects = joint.body_a == other || joint.body_b == other;
                if connects && !joint.collide_connected {
                    return false;
                }
            }
        }
        true
    }

    /// Creates the contact of a pair of shapes, ordering them by geometry rank.
    pub(crate) fn create_contact(&mut self, first: ShapeId, second: ShapeId) {
        let (Some(shape_first), Some(shape_second)) =
            (self.shapes.get(first), self.shapes.get(second))
        else {
            return;
        };
        let swap = geometry_rank(&shape_first.geometry) < geometry_rank(&shape_second.geometry);
        let (id_a, id_b) = if swap {
            (second, first)
        } else {
            (first, second)
        };
        let (shape_a, shape_b) = if swap {
            (shape_second, shape_first)
        } else {
            (shape_first, shape_second)
        };
        let (Some(body_a), Some(body_b)) =
            (self.bodies.get(shape_a.body), self.bodies.get(shape_b.body))
        else {
            return;
        };
        let mut contact = Contact::new(
            (id_a, shape_a),
            (id_b, shape_b),
            (shape_a.body, body_a),
            (shape_b.body, body_b),
        );
        contact.recycle = true;
        let (body_id_a, body_id_b) = (shape_a.body, shape_b.body);
        let id = self.contacts.insert(contact);
        for body_id in [body_id_a, body_id_b] {
            if let Some(body) = self.bodies.get_mut(body_id)
                && body.body_type != BodyType::Static
            {
                body.contacts.push(id);
            }
        }
    }

    /// The key of the unordered pair of shape slots.
    pub(crate) fn pair_key(a: ShapeId, b: ShapeId) -> u64 {
        let (lo, hi) = if a.index() < b.index() {
            (a.index(), b.index())
        } else {
            (b.index(), a.index())
        };
        ((lo as u64) << 32) | hi as u64
    }

    /// Destroys a contact, optionally waking the bodies it touched.
    pub(crate) fn destroy_contact(&mut self, id: ContactId, wake_bodies: bool) {
        let Some(contact) = self.contacts.get(id) else {
            return;
        };
        let (shape_a, shape_b, body_a, body_b) = (
            contact.shape_a,
            contact.shape_b,
            contact.body_a,
            contact.body_b,
        );
        let touching = contact.touching;
        let events = contact.enable_events;
        self.pair_set.remove(&Self::pair_key(shape_a, shape_b));
        if touching && events {
            self.contact_end_events
                .push(ContactEvent { shape_a, shape_b });
        }
        self.unlink_contact(id);
        for body_id in [body_a, body_b] {
            if let Some(body) = self.bodies.get_mut(body_id) {
                body.contacts.retain(|&c| c != id);
            }
        }
        self.contacts.remove(id);
        if wake_bodies && touching {
            self.wake_body(body_a);
            self.wake_body(body_b);
        }
    }

    /// The step context for sub-step settings; kept for reaction queries.
    pub(crate) fn make_context(&self, dt: f32, sub_steps: usize) -> StepContext {
        let sub_steps = sub_steps.max(1);
        let (inv_dt, h, inv_h) = if dt > 0.0 {
            (1.0 / dt, dt / sub_steps as f32, sub_steps as f32 / dt)
        } else {
            (0.0, 0.0, 0.0)
        };
        let contact_hertz = self.def.contact_hertz.min(0.125 * inv_h);
        StepContext {
            dt,
            inv_dt,
            h,
            inv_h,
            contact_softness: crate::math::Softness::new(
                contact_hertz,
                self.def.contact_damping_ratio,
                h,
            ),
            static_softness: crate::math::Softness::new(
                2.0 * contact_hertz,
                0.5 * self.def.contact_damping_ratio,
                h,
            ),
            restitution_threshold: self.def.restitution_threshold,
            contact_speed: self.def.contact_speed,
            max_linear_speed: self.def.maximum_linear_speed,
            enable_warm_starting: self.def.enable_warm_starting,
        }
    }

    /// The orientation of a body.
    pub fn body_rotation(&self, id: BodyId) -> Option<Quat> {
        self.bodies.get(id).map(|b| b.pose.rotation)
    }
}
