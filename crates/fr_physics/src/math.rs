//! Rigid poses, planes, soft constraint coefficients and the quaternion and matrix helpers of
//! the solver.
//!
//! Every expression keeps a fixed operand order and never fuses a multiply with an add, which is
//! what keeps a simulation reproducible bit for bit.

use std::f32::consts::PI;

use fr_core::{Mat3, Quat, Transform, Vec3};

/// A rigid transform: a rotation followed by a translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Where the origin of the local frame sits.
    pub position: Vec3,
    /// How the local frame is turned.
    pub rotation: Quat,
}

impl Pose {
    /// The pose that leaves everything where it is.
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };

    /// A pose from its parts.
    pub const fn new(position: Vec3, rotation: Quat) -> Self {
        Self { position, rotation }
    }

    /// A pose that only moves.
    pub const fn from_position(position: Vec3) -> Self {
        Self::new(position, Quat::IDENTITY)
    }

    /// A pose that only turns.
    pub const fn from_rotation(rotation: Quat) -> Self {
        Self::new(Vec3::ZERO, rotation)
    }

    /// The point `point` of the local frame in the parent frame.
    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        (self.rotation * point) + self.position
    }

    /// The point `point` of the parent frame in the local frame.
    pub fn inv_transform_point(&self, point: Vec3) -> Vec3 {
        inv_rotate(self.rotation, point - self.position)
    }

    /// The vector `vector` of the local frame in the parent frame.
    pub fn rotate(&self, vector: Vec3) -> Vec3 {
        self.rotation * vector
    }

    /// The vector `vector` of the parent frame in the local frame.
    pub fn inv_rotate(&self, vector: Vec3) -> Vec3 {
        inv_rotate(self.rotation, vector)
    }

    /// The pose `other`, given in this pose's frame, in this pose's parent frame.
    pub fn mul(&self, other: &Self) -> Self {
        Self {
            position: (self.rotation * other.position) + self.position,
            rotation: (self.rotation * other.rotation).normalize(),
        }
    }

    /// The pose `other`, given in this pose's parent frame, in this pose's frame.
    pub fn inv_mul(&self, other: &Self) -> Self {
        Self {
            position: inv_rotate(self.rotation, other.position - self.position),
            rotation: inv_mul_quat(self.rotation, other.rotation),
        }
    }

    /// The pose that undoes this one.
    pub fn inverse(&self) -> Self {
        let rotation = self.rotation.conjugate();
        Self {
            position: -(rotation * self.position),
            rotation,
        }
    }

    /// This pose as an engine transform with unit scale.
    pub fn to_transform(&self) -> Transform {
        Transform {
            translation: self.position,
            rotation: self.rotation,
            scale: Vec3::ONE,
        }
    }

    /// The pose of an engine transform, dropping its scale.
    pub fn from_transform(transform: &Transform) -> Self {
        Self::new(transform.translation, transform.rotation)
    }
}

impl Default for Pose {
    /// The identity pose.
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A plane with unit `normal` through the points `p` with `dot(normal, p) == offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// The unit normal.
    pub normal: Vec3,
    /// The signed distance of the plane from the origin along the normal.
    pub offset: f32,
}

impl Plane {
    /// The plane with unit `normal` through `point`.
    pub fn from_normal_and_point(normal: Vec3, point: Vec3) -> Self {
        Self {
            normal,
            offset: normal.dot(point),
        }
    }

    /// The signed distance of `point` in front of the plane.
    pub fn separation(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.offset
    }
}

/// The coefficients of a soft constraint, from a spring frequency and damping ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Softness {
    /// The velocity bias per unit of position error.
    pub bias_rate: f32,
    /// The fraction of the constraint mass that is used.
    pub mass_scale: f32,
    /// The fraction of the accumulated impulse that is softened away.
    pub impulse_scale: f32,
}

impl Softness {
    /// A rigid constraint.
    pub const RIGID: Self = Self {
        bias_rate: 0.0,
        mass_scale: 1.0,
        impulse_scale: 0.0,
    };

    /// The coefficients of a spring of `hertz` and damping ratio `zeta` over a sub-step `h`; a
    /// frequency of zero gives a constraint that does nothing.
    pub fn new(hertz: f32, zeta: f32, h: f32) -> Self {
        if hertz == 0.0 {
            return Self {
                bias_rate: 0.0,
                mass_scale: 0.0,
                impulse_scale: 0.0,
            };
        }
        let omega = 2.0 * PI * hertz;
        let a1 = 2.0 * zeta + h * omega;
        let a2 = h * omega * a1;
        let a3 = 1.0 / (1.0 + a2);
        Self {
            bias_rate: omega / a1,
            mass_scale: a2 * a3,
            impulse_scale: a3,
        }
    }
}

/// The smallest squared length that is still treated as a direction.
pub const MIN_LENGTH_SQUARED: f32 = 1000.0 * f32::MIN_POSITIVE;

/// `v` scaled to unit length, or zero when it is too short.
pub fn normalize(v: Vec3) -> Vec3 {
    let length = v.length();
    if length < f32::EPSILON {
        return Vec3::ZERO;
    }
    v * (1.0 / length)
}

/// The length of `v` and `v` scaled to unit length, or zero and the zero vector when it is too short.
pub fn length_and_normalize(v: Vec3) -> (f32, Vec3) {
    let length = v.length();
    if length < f32::EPSILON {
        return (0.0, Vec3::ZERO);
    }
    (length, v * (1.0 / length))
}

/// A unit vector perpendicular to the unit vector `a`.
pub fn perp(a: Vec3) -> Vec3 {
    let p = if !(-0.5..=0.5).contains(&a.x) {
        Vec3::new(a.y, -a.x, 0.0)
    } else {
        Vec3::new(0.0, a.z, -a.y)
    };
    normalize(p)
}

/// A vector perpendicular to the unit vector `v`, chosen to be stable across calls.
pub fn arbitrary_perp(v: Vec3) -> Vec3 {
    let a = 0.67;
    let b = -0.42;
    let p = if !(-0.5..=0.5).contains(&v.x) {
        Vec3::new(a * v.y + b * v.z, -a * v.x, -b * v.x)
    } else if !(-0.5..=0.5).contains(&v.y) {
        Vec3::new(a * v.y, -a * v.x + b * v.z, -b * v.y)
    } else {
        Vec3::new(a * v.z, b * v.z, -a * v.x - b * v.y)
    };
    normalize(p)
}

/// The vector `v` of the rotated frame of `q` in the parent frame.
pub fn rotate(q: Quat, v: Vec3) -> Vec3 {
    q * v
}

/// The vector `v` of the parent frame in the rotated frame of `q`.
pub fn inv_rotate(q: Quat, v: Vec3) -> Vec3 {
    q.conjugate() * v
}

/// `conjugate(a) * b`.
pub fn inv_mul_quat(a: Quat, b: Quat) -> Quat {
    a.conjugate() * b
}

/// The vector part of a quaternion.
pub fn quat_vector(q: Quat) -> Vec3 {
    Vec3::new(q.x, q.y, q.z)
}

/// The quaternion with vector part `v` and scalar part `s`.
pub fn quat_from_parts(v: Vec3, s: f32) -> Quat {
    Quat::from_xyzw(v.x, v.y, v.z, s)
}

/// The quaternion `q` scaled to unit length, or the identity when it is too short.
pub fn normalize_quat(q: Quat) -> Quat {
    let length_squared = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
    if length_squared < f32::EPSILON * f32::EPSILON {
        return Quat::IDENTITY;
    }
    let inverse = 1.0 / length_squared.sqrt();
    Quat::from_xyzw(q.x * inverse, q.y * inverse, q.z * inverse, q.w * inverse)
}

/// The four-dimensional dot product of two quaternions.
pub fn dot_quat(a: Quat, b: Quat) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w
}

/// The quaternion `q` with the sign of all four components flipped.
pub fn negate_quat(q: Quat) -> Quat {
    Quat::from_xyzw(-q.x, -q.y, -q.z, -q.w)
}

/// The Hamilton product `a * b` written out component by component.
pub fn mul_quat(a: Quat, b: Quat) -> Quat {
    let av = quat_vector(a);
    let bv = quat_vector(b);
    let v = Vec3::new(
        a.w * bv.x + b.w * av.x,
        a.w * bv.y + b.w * av.y,
        a.w * bv.z + b.w * av.z,
    ) + av.cross(bv);
    quat_from_parts(v, a.w * b.w - av.dot(bv))
}

/// `q1` turned by the angular displacement `delta_rotation` in radians, to first order.
pub fn integrate_rotation(q1: Quat, delta_rotation: Vec3) -> Quat {
    let half = delta_rotation * 0.5;
    let qd = mul_quat(quat_from_parts(half, 0.0), q1);
    let q2 = Quat::from_xyzw(q1.x + qd.x, q1.y + qd.y, q1.z + qd.z, qd.w + q1.w);
    normalize_quat(q2)
}

/// The pseudo angular velocity that turns `q` towards `target`, as `2 * (target - q) * conj(q)`.
pub fn delta_quat_to_rotation(q: Quat, target: Quat) -> Vec3 {
    let s = if dot_quat(q, target) < 0.0 {
        negate_quat(q)
    } else {
        q
    };
    let diff = Quat::from_xyzw(
        target.x - s.x,
        target.y - s.y,
        target.z - s.z,
        target.w - s.w,
    );
    let product = mul_quat(diff, s.conjugate());
    quat_vector(product) * 2.0
}

/// The twist of `q` around the z axis in `[-pi, pi]`.
pub fn twist_angle(q: Quat) -> f32 {
    let twist = if q.w < 0.0 {
        (-q.z).atan2(-q.w)
    } else {
        q.z.atan2(q.w)
    };
    twist * 2.0
}

/// The swing of `q` away from the z axis in `[0, pi]`.
pub fn swing_angle(q: Quat) -> f32 {
    let x = (q.z * q.z + q.w * q.w).sqrt();
    let y = (q.x * q.x + q.y * q.y).sqrt();
    2.0 * y.atan2(x)
}

/// The component-wise product that gives the extent of a rotating arm: `(a.y*b.z + a.z*b.y, ...)`.
pub fn modified_cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z + a.z * b.y,
        a.z * b.x + a.x * b.z,
        a.x * b.y + a.y * b.x,
    )
}

/// The cross product matrix of `v`.
pub fn skew(v: Vec3) -> Mat3 {
    Mat3::from_cols(
        Vec3::new(0.0, v.z, -v.y),
        Vec3::new(-v.z, 0.0, v.x),
        Vec3::new(v.y, -v.x, 0.0),
    )
}

/// The determinant of `m`.
pub fn det(m: Mat3) -> f32 {
    m.x_axis.dot(m.y_axis.cross(m.z_axis))
}

/// The inverse of `m`, or the zero matrix when it is singular.
pub fn invert_matrix(m: Mat3) -> Mat3 {
    let d = det(m);
    if d.abs() > MIN_LENGTH_SQUARED {
        let inv_det = 1.0 / d;
        let out = Mat3::from_cols(
            m.y_axis.cross(m.z_axis) * inv_det,
            m.z_axis.cross(m.x_axis) * inv_det,
            m.x_axis.cross(m.y_axis) * inv_det,
        );
        return out.transpose();
    }
    Mat3::ZERO
}

/// The inverse of the transpose of `m`, or the zero matrix when it is singular; for a symmetric
/// matrix this is the inverse.
pub fn invert_symmetric(m: Mat3) -> Mat3 {
    let d = det(m);
    if d.abs() > MIN_LENGTH_SQUARED {
        let inv_det = 1.0 / d;
        return Mat3::from_cols(
            m.y_axis.cross(m.z_axis) * inv_det,
            m.z_axis.cross(m.x_axis) * inv_det,
            m.x_axis.cross(m.y_axis) * inv_det,
        );
    }
    Mat3::ZERO
}

/// The solution `x` of `m * x = a`, or zero when `m` is singular.
pub fn solve3(m: Mat3, a: Vec3) -> Vec3 {
    let d = det(m);
    if d.abs() > MIN_LENGTH_SQUARED {
        let inv_det = 1.0 / d;
        let sx = m.y_axis.cross(m.z_axis);
        let sy = m.z_axis.cross(m.x_axis);
        let sz = m.x_axis.cross(m.y_axis);
        return Vec3::new(
            inv_det * sx.dot(a),
            inv_det * sy.dot(a),
            inv_det * sz.dot(a),
        );
    }
    Vec3::ZERO
}

/// The inverse of the symmetric 2x2 matrix `[[a, b], [b, c]]` as `(inv_a, inv_b, inv_c)`, or zeros.
pub fn invert2(a: f32, b: f32, c: f32) -> (f32, f32, f32) {
    let det = a * c - b * b;
    if det != 0.0 {
        let inv_det = 1.0 / det;
        (inv_det * c, -inv_det * b, inv_det * a)
    } else {
        (0.0, 0.0, 0.0)
    }
}

/// The product `rotation * inertia * transpose(rotation)`.
pub fn rotate_inertia(rotation: Mat3, inertia: Mat3) -> Mat3 {
    (rotation * inertia) * rotation.transpose()
}

/// The inertia of a body of `mass` about a point `origin` away from its centre of mass.
pub fn steiner(mass: f32, origin: Vec3) -> Mat3 {
    let ixx = mass * (origin.y * origin.y + origin.z * origin.z);
    let iyy = mass * (origin.x * origin.x + origin.z * origin.z);
    let izz = mass * (origin.x * origin.x + origin.y * origin.y);
    let ixy = -mass * origin.x * origin.y;
    let ixz = -mass * origin.x * origin.z;
    let iyz = -mass * origin.y * origin.z;
    Mat3::from_cols(
        Vec3::new(ixx, ixy, ixz),
        Vec3::new(ixy, iyy, iyz),
        Vec3::new(ixz, iyz, izz),
    )
}

/// The diagonal matrix with `a`, `b` and `c` on the diagonal.
pub fn diagonal(a: f32, b: f32, c: f32) -> Mat3 {
    Mat3::from_cols(
        Vec3::new(a, 0.0, 0.0),
        Vec3::new(0.0, b, 0.0),
        Vec3::new(0.0, 0.0, c),
    )
}

/// The inertia of a solid sphere about its centre.
pub fn sphere_inertia(mass: f32, radius: f32) -> Mat3 {
    let i = 0.4 * mass * radius * radius;
    diagonal(i, i, i)
}

/// The inertia of a solid cylinder about its centre, with its axis along y.
pub fn cylinder_inertia(mass: f32, radius: f32, height: f32) -> Mat3 {
    let ixx = mass * (3.0 * radius * radius + height * height) / 12.0;
    let iyy = 0.5 * mass * radius * radius;
    diagonal(ixx, iyy, ixx)
}

/// The component-wise absolute value.
pub fn abs(v: Vec3) -> Vec3 {
    Vec3::new(v.x.abs(), v.y.abs(), v.z.abs())
}

/// The velocity of the point at offset `r` of a body moving with `v` and turning with `w`.
pub fn point_velocity(v: Vec3, w: Vec3, r: Vec3) -> Vec3 {
    v + w.cross(r)
}

/// `a + s * b` evaluated as a multiply then an add.
pub fn mul_add(a: Vec3, s: f32, b: Vec3) -> Vec3 {
    Vec3::new(a.x + s * b.x, a.y + s * b.y, a.z + s * b.z)
}

/// `a - s * b` evaluated as a multiply then a subtract.
pub fn mul_sub(a: Vec3, s: f32, b: Vec3) -> Vec3 {
    Vec3::new(a.x - s * b.x, a.y - s * b.y, a.z - s * b.z)
}

/// Whether every component of `v` is finite.
pub fn is_valid(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}
