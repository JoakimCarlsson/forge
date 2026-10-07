//! The separating axis test between two hulls: face directions of both hulls and edge pairs.

use fr_math::{Mat3, Vec3};

use crate::constants::{NULL_INDEX, PARALLEL_EDGE_TOL, SPECULATIVE_DISTANCE};
use crate::hull::Hull;
use crate::math::{Pose, normalize};

/// Which kind of feature produced a separating axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AxisKind {
    /// No axis.
    #[default]
    Invalid,
    /// A face normal of hull A.
    FaceA,
    /// A face normal of hull B.
    FaceB,
    /// The cross product of an edge of A and an edge of B.
    EdgePair,
}

/// An axis found by the separating axis test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeparatingAxis {
    /// The unit axis pointing from A to B, in the frame of A.
    pub normal: Vec3,
    /// The signed distance along the axis between the hulls.
    pub separation: f32,
    /// The face, vertex or edge index on A, or [`NULL_INDEX`].
    pub index_a: usize,
    /// The face, vertex or edge index on B, or [`NULL_INDEX`].
    pub index_b: usize,
}

impl SeparatingAxis {
    /// An axis that separates nothing yet.
    const NONE: Self = Self {
        normal: Vec3::ZERO,
        separation: f32::NEG_INFINITY,
        index_a: NULL_INDEX,
        index_b: NULL_INDEX,
    };
}

/// The best axis of each family.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisQuery {
    /// The best face of A; `index_b` is the support vertex of B.
    pub face_a: SeparatingAxis,
    /// The best face of B; `index_a` is the support vertex of A.
    pub face_b: SeparatingAxis,
    /// The best edge pair.
    pub edge: SeparatingAxis,
    /// The family whose axis separated the hulls beyond the speculative distance.
    pub separated: AxisKind,
}

/// The best face of `hull_a` against the vertices of `hull_b`, or the first face that separates.
fn query_faces_a(
    hull_a: &Hull,
    hull_b: &Hull,
    xf_b: &Pose,
    matrix_inv: Mat3,
    early_return: bool,
) -> (SeparatingAxis, bool) {
    let mut best = SeparatingAxis::NONE;
    for (i, plane) in hull_a.planes().iter().enumerate() {
        let direction = -(matrix_inv * plane.normal);
        let vertex = hull_b.support_vertex(direction);
        let support = xf_b.transform_point(hull_b.points()[vertex]);
        let separation = plane.separation(support);
        if separation > best.separation {
            best = SeparatingAxis {
                normal: plane.normal,
                separation,
                index_a: i,
                index_b: vertex,
            };
            if separation > SPECULATIVE_DISTANCE && early_return {
                return (best, true);
            }
        }
    }
    (best, false)
}

/// The best face of `hull_b` against the vertices of `hull_a`, or the first face that separates.
fn query_faces_b(
    hull_a: &Hull,
    hull_b: &Hull,
    xf_b: &Pose,
    matrix: Mat3,
    early_return: bool,
) -> (SeparatingAxis, bool) {
    let mut best = SeparatingAxis::NONE;
    for (i, plane) in hull_b.planes().iter().enumerate() {
        let direction = -(matrix * plane.normal);
        let vertex = hull_a.support_vertex(direction);
        let support = xf_b.inv_transform_point(hull_a.points()[vertex]);
        let separation = plane.separation(support);
        if separation > best.separation {
            best = SeparatingAxis {
                normal: -plane.normal,
                separation,
                index_a: vertex,
                index_b: i,
            };
            if separation > SPECULATIVE_DISTANCE && early_return {
                return (best, true);
            }
        }
    }
    (best, false)
}

/// The best edge pair axis whose edges form a face of the Minkowski difference.
fn query_edges(
    hull_a: &Hull,
    hull_b: &Hull,
    xf_b: &Pose,
    matrix: Mat3,
    early_return: bool,
) -> (SeparatingAxis, bool) {
    let mut best = SeparatingAxis::NONE;
    let edges_a = hull_a.edges();
    let edges_b = hull_b.edges();
    let points_a = hull_a.points();
    let points_b = hull_b.points();
    let planes_a = hull_a.planes();
    let planes_b = hull_b.planes();
    let tolerance_squared = PARALLEL_EDGE_TOL * PARALLEL_EDGE_TOL;
    for index_a in (0..edges_a.len()).step_by(2) {
        let edge_a = edges_a[index_a];
        let twin_a = edges_a[index_a + 1];
        let p_a = points_a[edge_a.origin];
        let q_a = points_a[twin_a.origin];
        let e_a = q_a - p_a;
        let u_a = planes_a[edge_a.face].normal;
        let v_a = planes_a[twin_a.face].normal;
        for index_b in (0..edges_b.len()).step_by(2) {
            let edge_b = edges_b[index_b];
            let twin_b = edges_b[index_b + 1];
            let p_b = xf_b.transform_point(points_b[edge_b.origin]);
            let q_b = xf_b.transform_point(points_b[twin_b.origin]);
            let e_b = q_b - p_b;
            let u_b = matrix * planes_b[edge_b.face].normal;
            let v_b = matrix * planes_b[twin_b.face].normal;
            let cba = u_b.dot(e_a);
            let dba = v_b.dot(e_a);
            let adc = -u_a.dot(e_b);
            let bdc = -v_a.dot(e_b);
            if cba * dba < 0.0 && adc * bdc < 0.0 && cba * bdc > 0.0 {
                if (cba * cba).max(dba * dba) < tolerance_squared * e_a.length_squared() {
                    continue;
                }
                let t = cba / (cba - dba);
                let axis = normalize(u_b + (v_b - u_b) * t);
                let separation = axis.dot(q_a - q_b);
                if separation > best.separation {
                    best = SeparatingAxis {
                        normal: -axis,
                        separation,
                        index_a,
                        index_b,
                    };
                    if separation > SPECULATIVE_DISTANCE && early_return {
                        return (best, true);
                    }
                }
            }
        }
    }
    (best, false)
}

/// Runs the separating axis test of `hull_a` against `hull_b` placed by `xf_b` in the frame of
/// A. With `early_return` it stops at the first axis that separates the hulls beyond the
/// speculative distance and reports its family in [`AxisQuery::separated`].
pub fn compute_separating_axis(
    hull_a: &Hull,
    hull_b: &Hull,
    xf_b: &Pose,
    early_return: bool,
) -> AxisQuery {
    let matrix = Mat3::from_quat(xf_b.rotation);
    let matrix_inv = matrix.transpose();
    let mut query = AxisQuery {
        face_a: SeparatingAxis::NONE,
        face_b: SeparatingAxis::NONE,
        edge: SeparatingAxis::NONE,
        separated: AxisKind::Invalid,
    };
    let (face_a, separated) = query_faces_a(hull_a, hull_b, xf_b, matrix_inv, early_return);
    query.face_a = face_a;
    if separated {
        query.separated = AxisKind::FaceA;
        return query;
    }
    let (face_b, separated) = query_faces_b(hull_a, hull_b, xf_b, matrix, early_return);
    query.face_b = face_b;
    if separated {
        query.separated = AxisKind::FaceB;
        return query;
    }
    let (edge, separated) = query_edges(hull_a, hull_b, xf_b, matrix, early_return);
    query.edge = edge;
    if separated {
        query.separated = AxisKind::EdgePair;
    }
    query
}
