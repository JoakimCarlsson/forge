//! Contacts between pairs of shapes: the manifold with its warm starting impulses, its update
//! from the current poses and the recycling of manifolds while the pair barely moves.

use fr_core::{Mat3, Quat, Vec3};

use crate::body::{Body, BodyId, BodyType};
use crate::constants::MAX_MANIFOLD_POINTS;
use crate::distance::SimplexCache;
use crate::geometry::Geometry;
use crate::island::IslandId;
use crate::manifold::{
    LocalManifold, SatCache, collide_capsule_and_sphere, collide_capsules,
    collide_hull_and_capsule, collide_hull_and_sphere, collide_hulls, collide_spheres,
};
use crate::math::{Pose, abs, dot_quat, modified_cross};
use crate::shape::{Shape, ShapeId};
use crate::slot::Handle;

/// Marks [`ContactId`].
#[derive(Clone, Copy, Debug)]
pub struct ContactTag;

/// A handle to a contact of a world.
pub type ContactId = Handle<ContactTag>;

/// A point of a contact manifold.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ManifoldPoint {
    /// The contact position relative to the centre of mass of body A, in world axes.
    pub anchor_a: Vec3,
    /// The contact position relative to the centre of mass of body B, in world axes.
    pub anchor_b: Vec3,
    /// The signed distance between the surfaces; negative is penetration.
    pub separation: f32,
    /// The separation at the start of the step, from which sub-steps update it.
    pub base_separation: f32,
    /// The normal impulse of the last step, the warm starting value.
    pub normal_impulse: f32,
    /// The sum of the normal impulses applied over the last step.
    pub total_normal_impulse: f32,
    /// The normal approach velocity before the solve; negative when closing.
    pub normal_velocity: f32,
    /// The identity of the feature pair that made the point.
    pub feature_id: u32,
    /// Whether the point existed in the previous step.
    pub persisted: bool,
}

/// The contact manifold between two convex shapes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Manifold {
    /// The world unit normal from A to B.
    pub normal: Vec3,
    /// The points, of which `count` are in use.
    pub points: [ManifoldPoint; MAX_MANIFOLD_POINTS],
    /// The number of points in use.
    pub count: usize,
    /// The friction impulse of the last step in world axes.
    pub friction_impulse: Vec3,
    /// The twist friction impulse of the last step.
    pub twist_impulse: f32,
    /// The rolling resistance impulse of the last step.
    pub rolling_impulse: Vec3,
}

/// What a pair remembers to find its separating axis quickly.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContactCache {
    /// The simplex of the last distance query.
    pub simplex: SimplexCache,
    /// The axis of the last separating axis test.
    pub sat: SatCache,
}

/// A contact between two shapes.
#[derive(Clone, Debug)]
pub(crate) struct Contact {
    /// The first shape; the one with the higher geometry rank.
    pub(crate) shape_a: ShapeId,
    /// The second shape.
    pub(crate) shape_b: ShapeId,
    /// The body of the first shape.
    pub(crate) body_a: BodyId,
    /// The body of the second shape.
    pub(crate) body_b: BodyId,
    /// The manifold, empty while the shapes do not touch.
    pub(crate) manifold: Manifold,
    /// The mixed friction coefficient.
    pub(crate) friction: f32,
    /// The mixed restitution.
    pub(crate) restitution: f32,
    /// The mixed rolling resistance, as a torque coefficient.
    pub(crate) rolling_resistance: f32,
    /// The relative surface velocity, in world axes.
    pub(crate) tangent_velocity: Vec3,
    /// The cached separating axis state.
    pub(crate) cache: ContactCache,
    /// Whether the manifold has points.
    pub(crate) touching: bool,
    /// Whether the contact is between a static body and another body.
    pub(crate) is_static: bool,
    /// Whether begin and end touch events are reported.
    pub(crate) enable_events: bool,
    /// Whether both bodies allow recycling.
    pub(crate) recycle: bool,
    /// Whether the cached relative pose is valid.
    pub(crate) relative_pose_valid: bool,
    /// The orientation of body A when the manifold was built.
    pub(crate) cached_rotation_a: Quat,
    /// The orientation of body B when the manifold was built.
    pub(crate) cached_rotation_b: Quat,
    /// The pose of B in the frame of A when the manifold was built.
    pub(crate) cached_relative_pose: Pose,
    /// The island the touching contact links, if any.
    pub(crate) island: IslandId,
    /// The position in the island's contact list.
    pub(crate) island_index: usize,
}

/// What updating a contact found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContactChange {
    /// The bounds of the shapes no longer overlap; the contact should be destroyed.
    Disjoint,
    /// The previous manifold was reused.
    Recycled,
    /// The manifold was recomputed.
    Updated {
        /// Whether the shapes touch.
        touching: bool,
    },
}

/// The settings a contact update reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContactUpdateSettings {
    /// The distance a touching contact may be reused without a new manifold.
    pub(crate) recycle_distance: f32,
    /// The distance a non touching contact may be reused.
    pub(crate) recycle_distance_non_touching: f32,
}

/// The rank that decides which shape of a pair is A.
pub(crate) fn geometry_rank(geometry: &Geometry) -> u8 {
    match geometry {
        Geometry::Sphere(_) => 0,
        Geometry::Capsule(_) => 1,
        Geometry::Hull(_) => 2,
    }
}

/// The radius used to scale rolling resistance.
fn rolling_radius(geometry: &Geometry) -> f32 {
    match geometry {
        Geometry::Sphere(sphere) => sphere.radius,
        Geometry::Capsule(capsule) => capsule.radius,
        Geometry::Hull(hull) => 0.25 * hull.inner_radius(),
    }
}

impl Contact {
    /// A new contact between `shape_a` and `shape_b` on bodies `body_a` and `body_b`.
    pub(crate) fn new(
        shape_a: (ShapeId, &Shape),
        shape_b: (ShapeId, &Shape),
        body_a: (BodyId, &Body),
        body_b: (BodyId, &Body),
    ) -> Self {
        Self {
            shape_a: shape_a.0,
            shape_b: shape_b.0,
            body_a: body_a.0,
            body_b: body_b.0,
            manifold: Manifold::default(),
            friction: 0.0,
            restitution: 0.0,
            rolling_resistance: 0.0,
            tangent_velocity: Vec3::ZERO,
            cache: ContactCache::default(),
            touching: false,
            is_static: body_a.1.body_type == BodyType::Static
                || body_b.1.body_type == BodyType::Static,
            enable_events: shape_a.1.enable_contact_events || shape_b.1.enable_contact_events,
            recycle: true,
            relative_pose_valid: false,
            cached_rotation_a: Quat::IDENTITY,
            cached_rotation_b: Quat::IDENTITY,
            cached_relative_pose: Pose::IDENTITY,
            island: IslandId::NULL,
            island_index: usize::MAX,
        }
    }

    /// Updates the contact for the current poses of its bodies.
    pub(crate) fn collide(
        &mut self,
        shape_a: &Shape,
        shape_b: &Shape,
        body_a: &Body,
        body_b: &Body,
        settings: &ContactUpdateSettings,
    ) -> ContactChange {
        if !shape_a.fat_aabb.overlaps(&shape_b.fat_aabb) {
            self.touching = false;
            return ContactChange::Disjoint;
        }
        let was_touching = self.touching;
        let transform_a = body_a.pose;
        let transform_b = body_b.pose;
        let is_fast = body_a.is_fast || body_b.is_fast;
        let tolerance = if was_touching {
            settings.recycle_distance
        } else {
            settings.recycle_distance_non_touching
        };
        if !is_fast
            && settings.recycle_distance > 0.0
            && self.relative_pose_valid
            && self.recycle
            && self.try_recycle(body_a, body_b, tolerance)
        {
            return ContactChange::Recycled;
        }
        self.cached_rotation_a = transform_a.rotation;
        self.cached_rotation_b = transform_b.rotation;
        self.cached_relative_pose = transform_a.inv_mul(&transform_b);
        self.relative_pose_valid = true;
        let touching = self.update_manifold(shape_a, shape_b, body_a, body_b);
        self.touching = touching;
        if touching {
            for point in self.manifold.points.iter_mut().take(self.manifold.count) {
                point.base_separation = point.separation;
            }
        }
        ContactChange::Updated { touching }
    }

    /// Reuses the manifold when neither body moved far from where it was built, updating only the
    /// separations. Returns whether the manifold was reused.
    fn try_recycle(&mut self, body_a: &Body, body_b: &Body, tolerance: f32) -> bool {
        let transform_a = body_a.pose;
        let transform_b = body_b.pose;
        let angle_a = dot_quat(transform_a.rotation, self.cached_rotation_a);
        let angle_b = dot_quat(transform_b.rotation, self.cached_rotation_b);
        let angular_distance = (angle_a * angle_a).min(angle_b * angle_b);
        let xf = transform_a.inv_mul(&transform_b);
        let xfc = self.cached_relative_pose;
        let extent_a = if body_a.body_type != BodyType::Static {
            body_a.max_extent
        } else {
            Vec3::ZERO
        };
        let extent_b = if body_b.body_type != BodyType::Static {
            body_b.max_extent
        } else {
            Vec3::ZERO
        };
        let max_extent = extent_a.max(extent_b);
        let distance_squared = (xf.position - xfc.position).length_squared();
        if angular_distance <= crate::constants::CONTACT_RECYCLE_ANGULAR_DISTANCE
            || distance_squared >= tolerance * tolerance
        {
            return false;
        }
        let distance = distance_squared.sqrt();
        let slack = tolerance - distance;
        let qr = crate::math::inv_mul_quat(xfc.rotation, xf.rotation);
        let arc = modified_cross(abs(crate::math::quat_vector(qr)), max_extent);
        let arc_squared = 4.0 * arc.length_squared();
        if arc_squared >= slack * slack {
            return false;
        }
        let dq_a = transform_a.rotation * self.cached_rotation_a.conjugate();
        let dq_b = transform_b.rotation * self.cached_rotation_b.conjugate();
        let matrix_a = Mat3::from_quat(dq_a);
        let matrix_b = Mat3::from_quat(dq_b);
        let dc = body_b.center - body_a.center;
        let normal = self.manifold.normal;
        for point in self.manifold.points.iter_mut().take(self.manifold.count) {
            let r_a = matrix_a * point.anchor_a;
            let r_b = matrix_b * point.anchor_b;
            let dp = dc + (r_b - r_a);
            point.separation = point.base_separation + dp.dot(normal);
            point.normal_velocity = 0.0;
            point.persisted = true;
        }
        true
    }

    /// Recomputes the manifold; returns whether the shapes touch.
    fn update_manifold(
        &mut self,
        shape_a: &Shape,
        shape_b: &Shape,
        body_a: &Body,
        body_b: &Body,
    ) -> bool {
        let xf_a = body_a.pose;
        let xf_b = body_b.pose;
        let b_to_a = xf_a.inv_mul(&xf_b);
        let local = match (&shape_a.geometry, &shape_b.geometry) {
            (Geometry::Sphere(a), Geometry::Sphere(b)) => collide_spheres(a, b, &b_to_a),
            (Geometry::Capsule(a), Geometry::Sphere(b)) => {
                collide_capsule_and_sphere(a, b, &b_to_a)
            }
            (Geometry::Capsule(a), Geometry::Capsule(b)) => collide_capsules(a, b, &b_to_a),
            (Geometry::Hull(a), Geometry::Sphere(b)) => {
                collide_hull_and_sphere(a, b, &b_to_a, &mut self.cache.simplex)
            }
            (Geometry::Hull(a), Geometry::Capsule(b)) => {
                collide_hull_and_capsule(a, b, &b_to_a, &mut self.cache.simplex)
            }
            (Geometry::Hull(a), Geometry::Hull(b)) => {
                collide_hulls(a, b, &b_to_a, &mut self.cache.sat)
            }
            _ => LocalManifold::default(),
        };
        if local.count == 0 {
            self.manifold = Manifold::default();
            return false;
        }
        self.write_manifold(&local, &xf_a, &xf_b, body_a, body_b);
        self.mix_materials(shape_a, shape_b, &xf_a, &xf_b);
        true
    }

    /// Converts a local manifold to world anchors and carries impulses over by feature id.
    fn write_manifold(
        &mut self,
        local: &LocalManifold,
        xf_a: &Pose,
        xf_b: &Pose,
        body_a: &Body,
        body_b: &Body,
    ) {
        let old = self.manifold;
        let matrix_a = Mat3::from_quat(xf_a.rotation);
        let center_a = xf_a.rotate(body_a.local_center);
        let center_b = xf_b.rotate(body_b.local_center);
        let mut manifold = Manifold {
            normal: matrix_a * local.normal,
            count: local.count,
            friction_impulse: old.friction_impulse,
            twist_impulse: old.twist_impulse,
            rolling_impulse: old.rolling_impulse,
            ..Manifold::default()
        };
        let mut used = [false; MAX_MANIFOLD_POINTS];
        for i in 0..local.count {
            let source = &local.points[i];
            let anchor_a = matrix_a * source.point;
            let anchor_b = anchor_a + (xf_a.position - xf_b.position);
            let feature_id = source.pair.id();
            let mut point = ManifoldPoint {
                anchor_a: anchor_a - center_a,
                anchor_b: anchor_b - center_b,
                separation: source.separation,
                feature_id,
                ..ManifoldPoint::default()
            };
            for (j, old_point) in old.points.iter().enumerate().take(old.count) {
                if !used[j] && old_point.feature_id == feature_id {
                    point.normal_impulse = old_point.normal_impulse;
                    point.persisted = true;
                    used[j] = true;
                    break;
                }
            }
            manifold.points[i] = point;
        }
        self.manifold = manifold;
    }

    /// Mixes the materials of the two shapes into the contact.
    fn mix_materials(&mut self, shape_a: &Shape, shape_b: &Shape, xf_a: &Pose, xf_b: &Pose) {
        let a = &shape_a.material;
        let b = &shape_b.material;
        self.friction = (a.friction * b.friction).sqrt();
        self.restitution = a.restitution.max(b.restitution);
        self.tangent_velocity = xf_a.rotate(a.tangent_velocity) - xf_b.rotate(b.tangent_velocity);
        if a.rolling_resistance > 0.0 || b.rolling_resistance > 0.0 {
            let radius = rolling_radius(&shape_a.geometry).max(rolling_radius(&shape_b.geometry));
            self.rolling_resistance = a.rolling_resistance.max(b.rolling_resistance) * radius;
        } else {
            self.rolling_resistance = 0.0;
        }
    }
}
