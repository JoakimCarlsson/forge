//! The convex shapes a collider can have: sphere, capsule and hull, with their mass, bounds,
//! extents and ray casts in the local frame.

use std::f32::consts::PI;
use std::sync::Arc;

use fr_core::{Mat3, Quat, Vec3};

use crate::aabb::Aabb;
use crate::constants::LINEAR_SLOP;
use crate::hull::Hull;
use crate::math::{
    Pose, abs, cylinder_inertia, length_and_normalize, rotate_inertia, sphere_inertia, steiner,
};

/// A ball.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    /// The centre in the local frame.
    pub center: Vec3,
    /// The radius.
    pub radius: f32,
}

/// The set of points within `radius` of the segment between two centres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    /// The two ends of the core segment in the local frame.
    points: [Vec3; 2],
    /// The radius.
    pub radius: f32,
}

impl Capsule {
    /// A capsule from the two ends of its core segment and its radius.
    pub const fn new(center1: Vec3, center2: Vec3, radius: f32) -> Self {
        Self {
            points: [center1, center2],
            radius,
        }
    }

    /// The first end of the core segment.
    pub const fn center1(&self) -> Vec3 {
        self.points[0]
    }

    /// The second end of the core segment.
    pub const fn center2(&self) -> Vec3 {
        self.points[1]
    }

    /// Both ends of the core segment.
    pub const fn centers(&self) -> &[Vec3; 2] {
        &self.points
    }
}

/// The mass, centre and rotational inertia about the centre of a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassData {
    /// The mass.
    pub mass: f32,
    /// The centre of mass in the local frame.
    pub center: Vec3,
    /// The inertia tensor about the centre of mass.
    pub inertia: Mat3,
}

/// The point cloud and radius a distance query sees: the shape is the cloud's convex hull
/// grown by the radius.
#[derive(Clone, Copy, Debug)]
pub struct ShapeProxy<'a> {
    /// The points whose convex hull is the core.
    pub points: &'a [Vec3],
    /// How far the shape reaches beyond the core.
    pub radius: f32,
}

/// The nearest point where a ray enters a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// The fraction of the ray translation at the hit.
    pub fraction: f32,
    /// The hit point.
    pub point: Vec3,
    /// The surface normal at the hit.
    pub normal: Vec3,
}

/// How far a shape reaches about a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    /// The radius of the largest sphere about the point inside the shape.
    pub min: f32,
    /// The farthest reach along each axis.
    pub max: Vec3,
}

/// The shape of a collider.
#[derive(Clone, Debug)]
pub enum Geometry {
    /// A ball.
    Sphere(Sphere),
    /// A capsule.
    Capsule(Capsule),
    /// A convex hull, shared between colliders.
    Hull(Arc<Hull>),
}

impl Geometry {
    /// A ball of `radius` centred on the local origin.
    pub fn sphere(radius: f32) -> Self {
        Self::Sphere(Sphere {
            center: Vec3::ZERO,
            radius,
        })
    }

    /// A capsule along the local y axis with `half_height` of core on each side of the origin.
    pub fn capsule_y(half_height: f32, radius: f32) -> Self {
        Self::Capsule(Capsule::new(
            Vec3::new(0.0, -half_height, 0.0),
            Vec3::new(0.0, half_height, 0.0),
            radius,
        ))
    }

    /// A box of half sizes `half_extents` centred on the local origin.
    pub fn cuboid(half_extents: Vec3) -> Self {
        Self::Hull(Arc::new(Hull::cuboid(half_extents)))
    }

    /// The point cloud of the shape for distance queries.
    pub fn proxy(&self) -> ShapeProxy<'_> {
        match self {
            Self::Sphere(sphere) => ShapeProxy {
                points: std::slice::from_ref(&sphere.center),
                radius: sphere.radius,
            },
            Self::Capsule(capsule) => ShapeProxy {
                points: capsule.centers(),
                radius: capsule.radius,
            },
            Self::Hull(hull) => ShapeProxy {
                points: hull.points(),
                radius: 0.0,
            },
        }
    }

    /// The mass properties of the shape for `density`.
    pub fn compute_mass(&self, density: f32) -> MassData {
        match self {
            Self::Sphere(sphere) => {
                let radius = sphere.radius;
                let volume = 4.0 / 3.0 * PI * radius * radius * radius;
                let mass = volume * density;
                MassData {
                    mass,
                    center: sphere.center,
                    inertia: sphere_inertia(mass, radius),
                }
            }
            Self::Capsule(capsule) => capsule_mass(capsule, density),
            Self::Hull(hull) => MassData {
                mass: density * hull.volume(),
                center: hull.center(),
                inertia: hull.central_inertia() * density,
            },
        }
    }

    /// The bounds of the shape moved by `pose`.
    pub fn compute_aabb(&self, pose: &Pose) -> Aabb {
        match self {
            Self::Sphere(sphere) => {
                let center = pose.transform_point(sphere.center);
                Aabb::from_center_extents(center, Vec3::splat(sphere.radius))
            }
            Self::Capsule(capsule) => {
                let a = pose.transform_point(capsule.center1());
                let b = pose.transform_point(capsule.center2());
                let r = Vec3::splat(capsule.radius);
                Aabb::new(a.min(b) - r, a.max(b) + r)
            }
            Self::Hull(hull) => hull.compute_aabb(pose),
        }
    }

    /// The reach of the shape about the local point `origin`.
    pub fn extent(&self, origin: Vec3) -> Extent {
        match self {
            Self::Sphere(sphere) => Extent {
                min: sphere.radius,
                max: abs(sphere.center - origin) + Vec3::splat(sphere.radius),
            },
            Self::Capsule(capsule) => {
                let c1 = capsule.center1() - origin;
                let c2 = capsule.center2() - origin;
                Extent {
                    min: capsule.radius,
                    max: abs(c1).max(abs(c2)) + Vec3::splat(capsule.radius),
                }
            }
            Self::Hull(hull) => {
                let mut max = Vec3::ZERO;
                for &point in hull.points() {
                    max = max.max(abs(point - origin));
                }
                Extent {
                    min: hull.inner_radius(),
                    max,
                }
            }
        }
    }

    /// The distance from the local origin to the farthest point of the shape's centroid reach,
    /// which sizes the bounds margin of the shape.
    pub fn margin_radius(&self) -> f32 {
        match self {
            Self::Sphere(sphere) => sphere.radius,
            Self::Capsule(capsule) => {
                0.5 * (capsule.center2() - capsule.center1()).length() + capsule.radius
            }
            Self::Hull(hull) => {
                let mut max_squared = 0.0_f32;
                for &point in hull.points() {
                    max_squared = max_squared.max((point - hull.center()).length_squared());
                }
                max_squared.sqrt()
            }
        }
    }

    /// The reference point whose distance bounds the radius of the shape for sweeps.
    pub fn local_centroid(&self) -> Vec3 {
        match self {
            Self::Sphere(sphere) => sphere.center,
            Self::Capsule(capsule) => (capsule.center1() + capsule.center2()) * 0.5,
            Self::Hull(hull) => hull.center(),
        }
    }

    /// Casts a segment against the shape in its local frame.
    pub fn ray_cast(&self, origin: Vec3, translation: Vec3, max_fraction: f32) -> Option<RayHit> {
        match self {
            Self::Sphere(sphere) => ray_cast_sphere(sphere, origin, translation, max_fraction),
            Self::Capsule(capsule) => ray_cast_capsule(capsule, origin, translation, max_fraction),
            Self::Hull(hull) => {
                let (fraction, normal) = hull.ray_cast(origin, translation, max_fraction)?;
                Some(RayHit {
                    fraction,
                    point: origin + translation * fraction,
                    normal,
                })
            }
        }
    }
}

/// The mass properties of a capsule.
fn capsule_mass(capsule: &Capsule, density: f32) -> MassData {
    let c1 = capsule.center1();
    let c2 = capsule.center2();
    let r = capsule.radius;
    let cylinder_height = (c2 - c1).length();
    let cylinder_mass = PI * r * r * cylinder_height * density;
    let sphere_mass = (4.0 / 3.0) * PI * r * r * r * density;
    let mut inertia =
        cylinder_inertia(cylinder_mass, r, cylinder_height) + sphere_inertia(sphere_mass, r);
    let parallel = 0.125 * sphere_mass * (3.0 * r + 2.0 * cylinder_height) * cylinder_height;
    inertia.x_axis.x += parallel;
    inertia.z_axis.z += parallel;
    let mut rotation = Mat3::IDENTITY;
    if cylinder_height * cylinder_height > 1000.0 * f32::MIN_POSITIVE {
        let direction = (c2 - c1).normalize();
        rotation = Mat3::from_quat(Quat::from_rotation_arc(Vec3::Y, direction));
    }
    MassData {
        mass: sphere_mass + cylinder_mass,
        center: (c1 + c2) * 0.5,
        inertia: rotate_inertia(rotation, inertia),
    }
}

/// Casts a segment against a sphere; a segment that starts inside hits at fraction zero.
fn ray_cast_sphere(
    sphere: &Sphere,
    origin: Vec3,
    translation: Vec3,
    max_fraction: f32,
) -> Option<RayHit> {
    let s = origin - sphere.center;
    let rr = sphere.radius * sphere.radius;
    let inside = || {
        (s.length_squared() < rr).then_some(RayHit {
            fraction: 0.0,
            point: origin,
            normal: Vec3::ZERO,
        })
    };
    let (length, d) = length_and_normalize(translation);
    if length == 0.0 {
        return inside();
    }
    let t = -s.dot(d);
    let c = s + d * t;
    let cc = c.dot(c);
    if cc > rr {
        return None;
    }
    let h = (rr - cc).sqrt();
    let fraction = t - h;
    if fraction < 0.0 || max_fraction * length < fraction {
        return inside();
    }
    let hit_point = s + d * fraction;
    let normal = crate::math::normalize(hit_point);
    Some(RayHit {
        fraction: (fraction / length).min(max_fraction),
        point: sphere.center + normal * sphere.radius,
        normal,
    })
}

/// Casts a segment against a capsule; a segment that starts inside hits at fraction zero.
fn ray_cast_capsule(
    capsule: &Capsule,
    origin: Vec3,
    translation: Vec3,
    max_fraction: f32,
) -> Option<RayHit> {
    let c1 = capsule.center1();
    let c2 = capsule.center2();
    let r = capsule.radius;
    let d = c2 - c1;
    let tol = 0.01 * LINEAR_SLOP;
    let length_squared = d.length_squared();
    let sphere_at = |center: Vec3| Sphere { center, radius: r };
    if length_squared < tol * tol {
        return ray_cast_sphere(
            &sphere_at((c1 + c2) * 0.5),
            origin,
            translation,
            max_fraction,
        );
    }
    let s = origin - c1;
    let length = length_squared.sqrt();
    let axis = d * (1.0 / length);
    let u = s.dot(axis);
    let sc = s - axis * u;
    let sc2 = sc.length_squared();
    if sc2 < r * r {
        let u_clamped = u.clamp(0.0, length);
        let scp = s - axis * u_clamped;
        if scp.length_squared() < r * r {
            return Some(RayHit {
                fraction: 0.0,
                point: origin,
                normal: Vec3::ZERO,
            });
        }
        return ray_cast_sphere(
            &sphere_at(c1 + axis * u_clamped),
            origin,
            translation,
            max_fraction,
        );
    }
    let (ray_length, ray_axis) = length_and_normalize(translation);
    if ray_length == 0.0 {
        return None;
    }
    let v = u + max_fraction * translation.dot(axis);
    if (u < -r && v < -r) || (length + r < u && length + r < v) {
        return None;
    }
    let a12 = axis.dot(ray_axis);
    let det = 1.0 - a12 * a12;
    let tr = if det < f32::EPSILON {
        let perp = ray_axis - axis * a12;
        let perp2 = perp.length_squared();
        let beta = sc.dot(perp);
        let gamma = sc2 - r * r;
        let disc = beta * beta - perp2 * gamma;
        if beta >= 0.0 || disc < 0.0 {
            return None;
        }
        gamma / (-beta + disc.sqrt())
    } else {
        let inv_det = 1.0 / det;
        let sa1 = u;
        let sa2 = s.dot(ray_axis);
        let t1 = (sa1 - a12 * sa2) * inv_det;
        let t2 = (a12 * sa1 - sa2) * inv_det;
        let p1 = axis * t1;
        let p2 = s + ray_axis * t2;
        let g = p2 - p1;
        let g2 = g.length_squared();
        if g2 > r * r {
            return None;
        }
        t2 - ((r * r - g2) * inv_det).sqrt()
    };
    if tr < 0.0 || max_fraction * ray_length < tr {
        return None;
    }
    let tc = u + tr * a12;
    if tc < 0.0 {
        return ray_cast_sphere(&sphere_at(c1), origin, translation, max_fraction);
    }
    if length < tc {
        return ray_cast_sphere(&sphere_at(c2), origin, translation, max_fraction);
    }
    let p = s + ray_axis * tr;
    let normal = crate::math::normalize(p - axis * tc);
    Some(RayHit {
        fraction: (tr / ray_length).clamp(0.0, max_fraction),
        point: c1 + p,
        normal,
    })
}

/// The inertia of a solid of `mass` and central `inertia` seen from `origin` away from its centre.
pub fn inertia_about(mass: f32, inertia: Mat3, origin: Vec3) -> Mat3 {
    inertia + steiner(mass, origin)
}
