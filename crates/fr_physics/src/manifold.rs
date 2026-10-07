//! Contact manifolds between spheres, capsules and hulls in the frame of shape A.
//!
//! A manifold has one normal from A to B and up to four points, each with a signed separation,
//! where negative is penetration and small positive values are speculative. Hulls use the
//! separating axis test with clipping, the other pairs use GJK and closest point formulas.

use fr_core::Vec3;

use crate::constants::{
    LINEAR_SLOP, MAX_CLIP_POINTS, MAX_MANIFOLD_POINTS, MIN_CAPSULE_LENGTH, NULL_INDEX,
    SPECULATIVE_DISTANCE,
};
use crate::distance::{
    DistanceInput, SimplexCache, line_distance, point_to_segment, segment_distance, shape_distance,
};
use crate::geometry::{Capsule, ShapeProxy, Sphere};
use crate::hull::Hull;
use crate::math::{Plane, Pose, arbitrary_perp, length_and_normalize, mul_add, mul_sub, normalize};
use crate::sat::{AxisKind, SeparatingAxis, compute_separating_axis};

/// The owner of a feature of a contact point.
const OWNER_A: u8 = 0;

/// The owner of a feature of a contact point.
const OWNER_B: u8 = 1;

/// The two features whose contact produced a point, used to match points between steps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeaturePair {
    /// Which shape owns the first feature.
    pub owner1: u8,
    /// The index of the first feature.
    pub index1: u8,
    /// Which shape owns the second feature.
    pub owner2: u8,
    /// The index of the second feature.
    pub index2: u8,
}

impl FeaturePair {
    /// The pair of a point that has no distinguishing feature.
    pub const SINGLE: Self = Self {
        owner1: 0,
        index1: 0,
        owner2: 0,
        index2: 0,
    };

    /// A pair from two owners and indices.
    pub const fn new(owner1: u8, index1: usize, owner2: u8, index2: usize) -> Self {
        Self {
            owner1,
            index1: index1 as u8,
            owner2,
            index2: index2 as u8,
        }
    }

    /// The pair seen from the other shape, so that swapping the reference face does not change
    /// the identity of a point.
    pub const fn flipped(self) -> Self {
        Self {
            owner1: 1 - self.owner2,
            index1: self.index2,
            owner2: 1 - self.owner1,
            index2: self.index1,
        }
    }

    /// The pair packed into 32 bits.
    pub const fn id(self) -> u32 {
        ((self.owner1 as u32) << 24)
            | ((self.index1 as u32) << 16)
            | ((self.owner2 as u32) << 8)
            | (self.index2 as u32)
    }
}

/// A contact point in the frame of shape A.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LocalPoint {
    /// The contact position, halfway between the surfaces.
    pub point: Vec3,
    /// The signed distance between the surfaces along the normal.
    pub separation: f32,
    /// The features that made the point.
    pub pair: FeaturePair,
}

/// A manifold in the frame of shape A; empty when the shapes do not touch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LocalManifold {
    /// The unit normal from A to B.
    pub normal: Vec3,
    /// The points, of which `count` are in use.
    pub points: [LocalPoint; MAX_MANIFOLD_POINTS],
    /// The number of points in use.
    pub count: usize,
}

impl LocalManifold {
    /// A manifold of one point.
    fn single(normal: Vec3, point: Vec3, separation: f32) -> Self {
        let mut manifold = Self {
            normal,
            ..Self::default()
        };
        manifold.points[0] = LocalPoint {
            point,
            separation,
            pair: FeaturePair::SINGLE,
        };
        manifold.count = 1;
        manifold
    }

    /// A manifold of two points.
    fn double(normal: Vec3, first: LocalPoint, second: LocalPoint) -> Self {
        let mut manifold = Self {
            normal,
            ..Self::default()
        };
        manifold.points[0] = first;
        manifold.points[1] = second;
        manifold.count = 2;
        manifold
    }
}

/// What a hull pair remembers from the previous step to find its axis quickly.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SatCache {
    /// The family of the cached axis.
    pub kind: AxisKind,
    /// The index on A.
    pub index_a: u8,
    /// The index on B.
    pub index_b: u8,
    /// The separation of the cached axis.
    pub separation: f32,
    /// Whether the last query was answered from the cache.
    pub hit: bool,
}

/// A polygon vertex while clipping.
#[derive(Clone, Copy, Debug, Default)]
struct ClipVertex {
    /// The position in the frame of A.
    position: Vec3,
    /// The separation from the reference plane.
    separation: f32,
    /// The features at the vertex.
    pair: FeaturePair,
}

/// Whether the unit direction `normal` is long enough to use.
fn has_direction(distance_squared: f32) -> bool {
    distance_squared > 1000.0 * f32::MIN_POSITIVE
}

/// Clips a segment against the part of space behind `plane`.
fn clip_segment(segment: &mut [ClipVertex; 2], plane: Plane) -> usize {
    let vertex1 = segment[0];
    let vertex2 = segment[1];
    let distance1 = plane.separation(vertex1.position);
    let distance2 = plane.separation(vertex2.position);
    let mut count = 0;
    if distance1 <= 0.0 {
        segment[count] = vertex1;
        count += 1;
    }
    if distance2 <= 0.0 {
        segment[count] = vertex2;
        count += 1;
    }
    if distance1 * distance2 < 0.0 && count < 2 {
        let t = distance1 / (distance1 - distance2);
        let position = (vertex1.position * (1.0 - t)) + (vertex2.position * t);
        segment[count] = ClipVertex {
            position,
            separation: 0.0,
            pair: if distance1 > 0.0 {
                vertex1.pair
            } else {
                vertex2.pair
            },
        };
        count += 1;
    }
    count
}

/// The side planes of face `face` of `hull` with their loop edge index.
fn side_planes(hull: &Hull, face: usize) -> Vec<(Plane, usize)> {
    let ref_normal = hull.planes()[face].normal;
    let start = hull.faces()[face].edge;
    let mut planes = Vec::new();
    let mut edge_index = start;
    loop {
        let edge = hull.edges()[edge_index];
        let next = hull.edges()[edge.next];
        let vertex1 = hull.points()[edge.origin];
        let vertex2 = hull.points()[next.origin];
        let tangent = normalize(vertex2 - vertex1);
        let binormal = tangent.cross(ref_normal);
        planes.push((Plane::from_normal_and_point(binormal, vertex1), edge_index));
        edge_index = edge.next;
        if edge_index == start {
            break;
        }
    }
    planes
}

/// Clips a segment against the side planes of a hull face; returns the number of points kept.
fn clip_segment_to_hull_face(segment: &mut [ClipVertex; 2], hull: &Hull, face: usize) -> usize {
    for (plane, _) in side_planes(hull, face) {
        if clip_segment(segment, plane) < 2 {
            return 0;
        }
    }
    2
}

/// The core segment of capsule B in the frame of A as clip vertices.
fn capsule_segment(capsule: &Capsule, b_to_a: &Pose) -> [ClipVertex; 2] {
    [
        ClipVertex {
            position: b_to_a.transform_point(capsule.center1()),
            separation: 0.0,
            pair: FeaturePair::new(OWNER_A, 0, OWNER_A, 0),
        },
        ClipVertex {
            position: b_to_a.transform_point(capsule.center2()),
            separation: 0.0,
            pair: FeaturePair::new(OWNER_A, 1, OWNER_A, 1),
        },
    ]
}

/// The hull face most separating a capsule, with the capsule point that is deepest behind it.
fn query_face_direction_hull_and_capsule(
    hull: &Hull,
    capsule: &Capsule,
    b_to_a: &Pose,
) -> SeparatingAxis {
    let points = [
        b_to_a.transform_point(capsule.center1()),
        b_to_a.transform_point(capsule.center2()),
    ];
    let mut best = SeparatingAxis {
        normal: Vec3::ZERO,
        separation: f32::MIN,
        index_a: NULL_INDEX,
        index_b: NULL_INDEX,
    };
    for (face, plane) in hull.planes().iter().enumerate() {
        let vertex = crate::distance::proxy_support(&points, -plane.normal);
        let separation = plane.separation(points[vertex]);
        if separation > best.separation {
            best = SeparatingAxis {
                normal: plane.normal,
                separation,
                index_a: face,
                index_b: vertex,
            };
        }
    }
    best
}

/// The hull edge that best separates a capsule axis from the hull.
fn query_edge_direction_hull_and_capsule(
    hull: &Hull,
    capsule: &Capsule,
    b_to_a: &Pose,
) -> SeparatingAxis {
    let mut best = SeparatingAxis {
        normal: Vec3::ZERO,
        separation: f32::MIN,
        index_a: NULL_INDEX,
        index_b: NULL_INDEX,
    };
    let p_a = b_to_a.transform_point(capsule.center1());
    let q_a = b_to_a.transform_point(capsule.center2());
    let e_a = q_a - p_a;
    let squared_tolerance =
        crate::constants::PARALLEL_EDGE_TOL * crate::constants::PARALLEL_EDGE_TOL;
    let edges = hull.edges();
    for index in (0..edges.len()).step_by(2) {
        let edge = edges[index];
        let twin = edges[index + 1];
        let q_b = hull.points()[twin.origin];
        let u_b = hull.planes()[edge.face].normal;
        let v_b = hull.planes()[twin.face].normal;
        let cba = u_b.dot(e_a);
        let dba = v_b.dot(e_a);
        if cba * dba < 0.0 {
            if (cba * cba).max(dba * dba) < squared_tolerance * e_a.length_squared() {
                continue;
            }
            let t = cba / (cba - dba);
            let axis = normalize(u_b + (v_b - u_b) * t);
            let separation = axis.dot(q_a - q_b);
            if separation > best.separation {
                best = SeparatingAxis {
                    normal: axis,
                    separation,
                    index_a: 0,
                    index_b: index,
                };
            }
        }
    }
    best
}

/// The manifold of two spheres.
pub fn collide_spheres(sphere_a: &Sphere, sphere_b: &Sphere, b_to_a: &Pose) -> LocalManifold {
    let center1 = sphere_a.center;
    let center2 = b_to_a.transform_point(sphere_b.center);
    let total_radius = sphere_a.radius + sphere_b.radius;
    let offset = center2 - center1;
    let distance_squared = offset.length_squared();
    if distance_squared > total_radius * total_radius {
        return LocalManifold::default();
    }
    let mut normal = Vec3::Y;
    let distance = distance_squared.sqrt();
    if has_direction(distance * distance) {
        normal = offset * (1.0 / distance);
    }
    let point = (mul_sub(
        mul_add(center1, sphere_a.radius, normal) + center2,
        sphere_b.radius,
        normal,
    )) * 0.5;
    LocalManifold::single(normal, point, distance - total_radius)
}

/// The manifold of a capsule A and a sphere B.
pub fn collide_capsule_and_sphere(
    capsule_a: &Capsule,
    sphere_b: &Sphere,
    b_to_a: &Pose,
) -> LocalManifold {
    let center = b_to_a.transform_point(sphere_b.center);
    let total_radius = sphere_b.radius + capsule_a.radius;
    let closest = point_to_segment(capsule_a.center1(), capsule_a.center2(), center);
    let offset = center - closest;
    let distance_squared = offset.length_squared();
    if distance_squared > total_radius * total_radius {
        return LocalManifold::default();
    }
    let mut normal = Vec3::Y;
    let distance = distance_squared.sqrt();
    if has_direction(distance * distance) {
        normal = offset * (1.0 / distance);
    }
    let point = mul_add(
        mul_sub(center, sphere_b.radius, normal) + closest,
        capsule_a.radius,
        normal,
    ) * 0.5;
    LocalManifold::single(normal, point, distance - total_radius)
}

/// The manifold of a hull A and a sphere B.
pub fn collide_hull_and_sphere(
    hull_a: &Hull,
    sphere_b: &Sphere,
    b_to_a: &Pose,
    cache: &mut SimplexCache,
) -> LocalManifold {
    let center = b_to_a.transform_point(sphere_b.center);
    let input = DistanceInput {
        proxy_a: ShapeProxy {
            points: hull_a.points(),
            radius: 0.0,
        },
        proxy_b: ShapeProxy {
            points: std::slice::from_ref(&center),
            radius: 0.0,
        },
        transform: Pose::IDENTITY,
        use_radii: false,
    };
    let radius_a = 0.0;
    let radius_b = sphere_b.radius;
    let radius = radius_a + radius_b;
    let output = shape_distance(&input, cache);
    if output.distance > radius + SPECULATIVE_DISTANCE {
        *cache = SimplexCache::default();
        return LocalManifold::default();
    }
    if output.distance > 100.0 * f32::EPSILON {
        let normal = normalize(output.point_b - output.point_a);
        let c_a = mul_add(
            center,
            radius_a - (center - output.point_a).dot(normal),
            normal,
        );
        let c_b = mul_sub(center, radius_b, normal);
        let point = c_a + (c_b - c_a) * 0.5;
        return LocalManifold::single(normal, point, output.distance - radius);
    }
    let mut best_index = 0;
    let mut best_distance = f32::MIN;
    for (index, plane) in hull_a.planes().iter().enumerate() {
        let distance = plane.separation(center);
        if distance > best_distance {
            best_index = index;
            best_distance = distance;
        }
    }
    let normal = hull_a.planes()[best_index].normal;
    let c_a = mul_add(
        center,
        radius_a - (center - output.point_a).dot(normal),
        normal,
    );
    let c_b = mul_sub(center, radius_b, normal);
    let point = c_a + (c_b - c_a) * 0.5;
    LocalManifold::single(normal, point, best_distance - radius)
}

/// The manifold of two capsules.
pub fn collide_capsules(capsule_a: &Capsule, capsule_b: &Capsule, b_to_a: &Pose) -> LocalManifold {
    let center_a1 = capsule_a.center1();
    let center_a2 = capsule_a.center2();
    let center_b1 = b_to_a.transform_point(capsule_b.center1());
    let center_b2 = b_to_a.transform_point(capsule_b.center2());
    let radius = capsule_a.radius + capsule_b.radius;
    let max_distance = radius + SPECULATIVE_DISTANCE;
    let result = segment_distance(center_a1, center_a2, center_b1, center_b2);
    let offset = result.point2 - result.point1;
    let distance_squared = offset.length_squared();
    let min_distance = 0.01 * LINEAR_SLOP;
    if distance_squared > max_distance * max_distance
        || distance_squared < min_distance * min_distance
    {
        return LocalManifold::default();
    }
    let (length_a, edge_a) = length_and_normalize(center_a2 - center_a1);
    if length_a < MIN_CAPSULE_LENGTH {
        return LocalManifold::default();
    }
    let (length_b, edge_b) = length_and_normalize(center_b2 - center_b1);
    if length_b < MIN_CAPSULE_LENGTH {
        return LocalManifold::default();
    }
    let alpha_tolerance_squared = 0.05 * 0.05;
    let axis = edge_a.cross(edge_b);
    if axis.length_squared() < alpha_tolerance_squared {
        let planes_a = [
            Plane {
                normal: -edge_a,
                offset: -edge_a.dot(center_a1),
            },
            Plane {
                normal: edge_a,
                offset: edge_a.dot(center_a2),
            },
        ];
        let mut vertices_b = capsule_segment(capsule_b, b_to_a);
        let mut count = clip_segment(&mut vertices_b, planes_a[0]);
        if count == 2 {
            count = clip_segment(&mut vertices_b, planes_a[1]);
        }
        if count == 2 {
            let closest1 = point_to_segment(center_a1, center_a2, vertices_b[0].position);
            let closest2 = point_to_segment(center_a1, center_a2, vertices_b[1].position);
            let distance1 = (closest1 - vertices_b[0].position).length();
            let distance2 = (closest2 - vertices_b[1].position).length();
            if distance1 <= radius && distance2 <= radius {
                if distance1 < min_distance || distance2 < min_distance {
                    return LocalManifold::default();
                }
                let normal1 = (vertices_b[0].position - closest1) * (1.0 / distance1);
                let normal2 = (vertices_b[1].position - closest2) * (1.0 / distance2);
                let normal = normalize(normal1 + normal2);
                let radius_a = capsule_a.radius;
                let radius_b = capsule_b.radius;
                let point1 = mul_sub(
                    mul_add(vertices_b[0].position, radius_a, normal1) + closest1,
                    radius_b,
                    normal,
                ) * 0.5;
                let point2 = mul_sub(
                    mul_add(vertices_b[1].position, radius_a, normal2) + closest2,
                    radius_b,
                    normal,
                ) * 0.5;
                return LocalManifold::double(
                    normal,
                    LocalPoint {
                        point: point1,
                        separation: distance1 - radius,
                        pair: vertices_b[0].pair,
                    },
                    LocalPoint {
                        point: point2,
                        separation: distance2 - radius,
                        pair: vertices_b[1].pair,
                    },
                );
            }
        }
    }
    let (distance, normal) = length_and_normalize(offset);
    let point = mul_sub(
        mul_add(result.point1, capsule_a.radius, normal) + result.point2,
        capsule_b.radius,
        normal,
    ) * 0.5;
    LocalManifold::single(normal, point, distance - radius)
}

/// Builds a two point manifold from a capsule clipped to the reference face of a hull.
fn hull_face_capsule_points(
    hull_a: &Hull,
    capsule_b: &Capsule,
    segment: &[ClipVertex; 2],
    ref_face: usize,
    allowed: f32,
) -> Option<LocalManifold> {
    let ref_plane = hull_a.planes()[ref_face];
    let distance1 = ref_plane.separation(segment[0].position);
    let distance2 = ref_plane.separation(segment[1].position);
    if distance1 <= allowed || distance2 <= allowed {
        let normal = ref_plane.normal;
        let point1 = mul_sub(
            segment[0].position,
            0.5 * (capsule_b.radius + distance1),
            normal,
        );
        let point2 = mul_sub(
            segment[1].position,
            0.5 * (capsule_b.radius + distance2),
            normal,
        );
        return Some(LocalManifold::double(
            normal,
            LocalPoint {
                point: point1,
                separation: distance1 - capsule_b.radius,
                pair: segment[0].pair,
            },
            LocalPoint {
                point: point2,
                separation: distance2 - capsule_b.radius,
                pair: segment[1].pair,
            },
        ));
    }
    None
}

/// Builds a one point manifold from the closest points of a hull edge and a capsule axis.
fn build_hull_and_capsule_edge_contact(
    hull_a: &Hull,
    capsule_b: &Capsule,
    b_to_a: &Pose,
    query: &SeparatingAxis,
) -> Option<LocalManifold> {
    let pc = b_to_a.transform_point(capsule_b.center1());
    let qc = b_to_a.transform_point(capsule_b.center2());
    let ec = qc - pc;
    let edge2 = hull_a.edges()[query.index_b];
    let twin2 = hull_a.edges()[edge2.twin];
    let ph = hull_a.points()[edge2.origin];
    let qh = hull_a.points()[twin2.origin];
    let eh = qh - ph;
    let normal = query.normal;
    let result = line_distance(ph, eh, pc, ec);
    if !result.is_within_segments() {
        return None;
    }
    let point = (mul_sub(result.point1, capsule_b.radius, normal) + result.point2) * 0.5;
    let separation = normal.dot(result.point2 - result.point1);
    let mut manifold = LocalManifold::single(normal, point, separation - capsule_b.radius);
    manifold.points[0].pair = FeaturePair::new(OWNER_A, query.index_a, OWNER_B, query.index_b);
    Some(manifold)
}

/// The manifold of a hull A and a capsule B.
pub fn collide_hull_and_capsule(
    hull_a: &Hull,
    capsule_b: &Capsule,
    b_to_a: &Pose,
    cache: &mut SimplexCache,
) -> LocalManifold {
    let input = DistanceInput {
        proxy_a: ShapeProxy {
            points: hull_a.points(),
            radius: 0.0,
        },
        proxy_b: ShapeProxy {
            points: capsule_b.centers(),
            radius: 0.0,
        },
        transform: *b_to_a,
        use_radii: false,
    };
    let output = shape_distance(&input, cache);
    let radius = capsule_b.radius;
    if output.distance > radius + SPECULATIVE_DISTANCE {
        *cache = SimplexCache::default();
        return LocalManifold::default();
    }
    if output.distance > 100.0 * f32::EPSILON {
        let delta = output.normal;
        let ref_face = hull_a.support_face(delta);
        let ref_plane = hull_a.planes()[ref_face];
        if ref_plane.normal.dot(delta).abs() > 0.998 {
            let mut segment = capsule_segment(capsule_b, b_to_a);
            if clip_segment_to_hull_face(&mut segment, hull_a, ref_face) == 2
                && let Some(manifold) = hull_face_capsule_points(
                    hull_a,
                    capsule_b,
                    &segment,
                    ref_face,
                    radius + SPECULATIVE_DISTANCE,
                )
            {
                return manifold;
            }
        }
        let point = (mul_sub(output.point_a, radius, delta) + output.point_b) * 0.5;
        return LocalManifold::single(delta, point, output.distance - radius);
    }
    let face_query = query_face_direction_hull_and_capsule(hull_a, capsule_b, b_to_a);
    if face_query.separation > radius {
        return LocalManifold::default();
    }
    let edge_query = query_edge_direction_hull_and_capsule(hull_a, capsule_b, b_to_a);
    if edge_query.separation > radius {
        return LocalManifold::default();
    }
    let mut face_separation = face_query.separation - radius;
    let mut manifold = LocalManifold::default();
    let mut segment = capsule_segment(capsule_b, b_to_a);
    if clip_segment_to_hull_face(&mut segment, hull_a, face_query.index_a) == 2
        && let Some(built) = hull_face_capsule_points(
            hull_a,
            capsule_b,
            &segment,
            face_query.index_a,
            SPECULATIVE_DISTANCE,
        )
    {
        manifold = built;
        face_separation = manifold.points[0]
            .separation
            .min(manifold.points[1].separation);
    }
    if edge_query.index_a == NULL_INDEX {
        return manifold;
    }
    let edge_separation = edge_query.separation - radius;
    if (manifold.count == 0 || edge_separation > face_separation + LINEAR_SLOP)
        && let Some(edge) =
            build_hull_and_capsule_edge_contact(hull_a, capsule_b, b_to_a, &edge_query)
    {
        return edge;
    }
    manifold
}

/// Reduces up to [`MAX_CLIP_POINTS`] candidate points to at most four that span the contact.
fn reduce_manifold_points(
    manifold: &mut LocalManifold,
    points: &mut [LocalPoint],
    mut count: usize,
) {
    if count <= MAX_MANIFOLD_POINTS {
        manifold.points[..count].copy_from_slice(&points[..count]);
        manifold.count = count;
        return;
    }
    let normal = manifold.normal;
    let tol_squared = SPECULATIVE_DISTANCE * SPECULATIVE_DISTANCE;
    let bias = 0.95;
    let mut best_index = NULL_INDEX;
    let mut best_score = f32::MIN;
    let search_direction = arbitrary_perp(normal);
    for (index, point) in points.iter().enumerate().take(count) {
        if point.separation > SPECULATIVE_DISTANCE {
            continue;
        }
        let score = -point.separation + search_direction.dot(point.point);
        if bias * score > best_score {
            best_index = index;
            best_score = score;
        }
    }
    if best_index == NULL_INDEX {
        manifold.count = 0;
        return;
    }
    manifold.points[0] = points[best_index];
    manifold.count = 1;
    points[best_index] = points[count - 1];
    count -= 1;
    let a = manifold.points[0].point;
    best_score = 0.0;
    best_index = NULL_INDEX;
    for (index, point) in points.iter().enumerate().take(count) {
        let d = point.point - a;
        let v = mul_sub(d, d.dot(normal), normal);
        let distance_squared = v.length_squared();
        let separation = (-point.separation).max(0.0);
        let score = distance_squared + 4.0 * separation * separation;
        if bias * score > best_score {
            best_score = score;
            best_index = index;
        }
    }
    if best_score < tol_squared {
        return;
    }
    manifold.points[1] = points[best_index];
    manifold.count = 2;
    points[best_index] = points[count - 1];
    count -= 1;
    let b = manifold.points[1].point;
    best_score = tol_squared;
    best_index = NULL_INDEX;
    let mut best_signed_area = 0.0;
    let ba = b - a;
    for (index, point) in points.iter().enumerate().take(count) {
        let signed_area = normal.dot(ba.cross(point.point - a));
        let score = signed_area.abs();
        if bias * score >= best_score {
            best_score = score;
            best_index = index;
            best_signed_area = signed_area;
        }
    }
    if best_index == NULL_INDEX {
        return;
    }
    manifold.points[2] = points[best_index];
    manifold.count = 3;
    points[best_index] = points[count - 1];
    count -= 1;
    let c = manifold.points[2].point;
    best_score = tol_squared;
    best_index = NULL_INDEX;
    let sign = if best_signed_area < 0.0 { -1.0 } else { 1.0 };
    for (index, point) in points.iter().enumerate().take(count) {
        let p = point.point;
        let u1 = sign * normal.dot((p - a).cross(ba));
        let u2 = sign * normal.dot((p - b).cross(c - b));
        let u3 = sign * normal.dot((p - c).cross(a - c));
        let score = u1.max(u2.max(u3));
        if bias * score > best_score {
            best_score = score;
            best_index = index;
        }
    }
    if best_index != NULL_INDEX {
        manifold.points[3] = points[best_index];
        manifold.count = 4;
    }
}

/// The face of `hull` to use as the incident face against a reference normal, found from the
/// edges around the closest vertex.
fn find_incident_face(hull: &Hull, ref_normal: Vec3, vertex_index: usize) -> usize {
    let edges = hull.edges();
    let points = hull.points();
    let start = hull.vertices()[vertex_index].edge;
    let origin = points[edges[start].origin];
    let mut min_edge = start;
    let mut min_projection = f32::MAX;
    let mut edge_index = start;
    loop {
        let edge = edges[edge_index];
        let twin = edges[edge.twin];
        let axis = normalize(points[twin.origin] - origin);
        let projection = axis.dot(ref_normal).abs();
        if projection < min_projection {
            min_edge = edge_index;
            min_projection = projection;
        }
        edge_index = twin.next;
        if edge_index == start {
            break;
        }
    }
    let edge = edges[min_edge];
    let face1 = edge.face;
    let face2 = edges[edge.twin].face;
    let planes = hull.planes();
    if planes[face1].normal.dot(ref_normal) < planes[face2].normal.dot(ref_normal) {
        face1
    } else {
        face2
    }
}

/// The incident polygon of `hull` in the frame of A, with separations from `ref_plane`.
fn build_polygon(
    out: &mut [ClipVertex; MAX_CLIP_POINTS],
    transform: &Pose,
    hull: &Hull,
    face: usize,
    ref_plane: Plane,
) -> usize {
    let start = hull.faces()[face].edge;
    let mut edge_index = start;
    let mut count = 0;
    loop {
        let edge = hull.edges()[edge_index];
        let next_index = edge.next;
        let next = hull.edges()[next_index];
        let position = transform.transform_point(hull.points()[next.origin]);
        out[count] = ClipVertex {
            position,
            separation: ref_plane.separation(position),
            pair: FeaturePair::new(OWNER_B, edge_index, OWNER_B, next_index),
        };
        count += 1;
        edge_index = next_index;
        if edge_index == start || count >= MAX_CLIP_POINTS {
            break;
        }
    }
    count
}

/// Clips a polygon against one side plane of the reference face.
fn clip_polygon(
    out: &mut [ClipVertex; MAX_CLIP_POINTS],
    polygon: &[ClipVertex; MAX_CLIP_POINTS],
    count: usize,
    clip_plane: Plane,
    edge: usize,
    ref_plane: Plane,
) -> usize {
    let mut vertex1 = polygon[count - 1];
    let mut distance1 = clip_plane.separation(vertex1.position);
    let mut out_count = 0;
    for &vertex2 in polygon.iter().take(count) {
        let distance2 = clip_plane.separation(vertex2.position);
        if out_count + 2 > MAX_CLIP_POINTS {
            break;
        }
        if distance1 <= 0.0 && distance2 <= 0.0 {
            out[out_count] = vertex2;
            out_count += 1;
        } else if distance1 <= 0.0 && distance2 > 0.0 {
            let fraction = distance1 / (distance1 - distance2);
            let position = mul_add(
                vertex1.position,
                fraction,
                vertex2.position - vertex1.position,
            );
            let mut pair = vertex2.pair;
            pair.owner2 = OWNER_A;
            pair.index2 = edge as u8;
            out[out_count] = ClipVertex {
                position,
                separation: ref_plane.separation(position),
                pair,
            };
            out_count += 1;
        } else if distance2 <= 0.0 && distance1 > 0.0 {
            let fraction = distance1 / (distance1 - distance2);
            let position = mul_add(
                vertex1.position,
                fraction,
                vertex2.position - vertex1.position,
            );
            let mut pair = vertex1.pair;
            pair.owner1 = OWNER_A;
            pair.index1 = edge as u8;
            out[out_count] = ClipVertex {
                position,
                separation: ref_plane.separation(position),
                pair,
            };
            out_count += 1;
            out[out_count] = vertex2;
            out_count += 1;
        }
        vertex1 = vertex2;
        distance1 = distance2;
    }
    out_count
}

/// The clip result of a reference face: the candidate points and the smallest separation.
fn build_face_a_contact(
    hull_a: &Hull,
    hull_b: &Hull,
    b_to_a: &Pose,
    ref_face: usize,
    vertex_b: usize,
    cache: &mut SatCache,
) -> LocalManifold {
    let ref_plane = hull_a.planes()[ref_face];
    let ref_normal_in_b = b_to_a.inv_rotate(ref_plane.normal);
    let inc_face = find_incident_face(hull_b, ref_normal_in_b, vertex_b);
    let mut buffer1 = [ClipVertex::default(); MAX_CLIP_POINTS];
    let mut buffer2 = [ClipVertex::default(); MAX_CLIP_POINTS];
    let mut count = build_polygon(&mut buffer1, b_to_a, hull_b, inc_face, ref_plane);
    let mut input = &mut buffer1;
    let mut output = &mut buffer2;
    for (clip_plane, edge_index) in side_planes(hull_a, ref_face) {
        count = clip_polygon(output, input, count, clip_plane, edge_index, ref_plane);
        std::mem::swap(&mut input, &mut output);
        if count < 3 {
            *cache = SatCache::default();
            return LocalManifold::default();
        }
    }
    let mut points = [LocalPoint::default(); MAX_CLIP_POINTS];
    let mut min_separation = f32::MAX;
    let mut manifold = LocalManifold {
        normal: ref_plane.normal,
        ..LocalManifold::default()
    };
    for (index, clip) in input.iter().enumerate().take(count) {
        points[index] = LocalPoint {
            point: mul_sub(clip.position, 0.5 * clip.separation, ref_plane.normal),
            separation: clip.separation,
            pair: clip.pair,
        };
        min_separation = min_separation.min(clip.separation);
    }
    if min_separation >= SPECULATIVE_DISTANCE {
        *cache = SatCache::default();
        return LocalManifold::default();
    }
    reduce_manifold_points(&mut manifold, &mut points, count);
    cache.separation = min_separation;
    cache.kind = AxisKind::FaceA;
    cache.index_a = ref_face as u8;
    cache.index_b = vertex_b as u8;
    manifold
}

/// The manifold when the reference face belongs to B, expressed in the frame of A.
fn build_face_b_contact(
    hull_a: &Hull,
    hull_b: &Hull,
    b_to_a: &Pose,
    vertex_a: usize,
    ref_face: usize,
    cache: &mut SatCache,
) -> LocalManifold {
    let a_to_b = b_to_a.inverse();
    let mut manifold = build_face_a_contact(hull_b, hull_a, &a_to_b, ref_face, vertex_a, cache);
    if manifold.count == 0 {
        *cache = SatCache::default();
        return manifold;
    }
    manifold.normal = -b_to_a.rotate(manifold.normal);
    for point in manifold.points.iter_mut().take(manifold.count) {
        point.point = b_to_a.transform_point(point.point);
        point.pair = point.pair.flipped();
    }
    cache.kind = AxisKind::FaceB;
    cache.index_a = vertex_a as u8;
    cache.index_b = ref_face as u8;
    manifold
}

/// The manifold of one point between two hull edges that realise `query`.
fn build_edge_contact(
    hull_a: &Hull,
    hull_b: &Hull,
    b_to_a: &Pose,
    query: &SeparatingAxis,
    cache: &mut SatCache,
) -> LocalManifold {
    let edge_a = hull_a.edges()[query.index_a];
    let twin_a = hull_a.edges()[edge_a.twin];
    let p_a = hull_a.points()[edge_a.origin];
    let q_a = hull_a.points()[twin_a.origin];
    let e_a = q_a - p_a;
    let edge_b = hull_b.edges()[query.index_b];
    let twin_b = hull_b.edges()[edge_b.twin];
    let p_b = b_to_a.transform_point(hull_b.points()[edge_b.origin]);
    let q_b = b_to_a.transform_point(hull_b.points()[twin_b.origin]);
    let e_b = q_b - p_b;
    let normal = query.normal;
    let result = line_distance(p_a, e_a, p_b, e_b);
    if !result.is_within_segments() {
        *cache = SatCache::default();
        return LocalManifold::default();
    }
    let separation = normal.dot(result.point2 - result.point1);
    let point = (result.point1 + result.point2) * 0.5;
    let mut manifold = LocalManifold::single(normal, point, separation);
    manifold.points[0].pair = FeaturePair::new(OWNER_A, query.index_a, OWNER_B, query.index_b);
    cache.separation = separation;
    cache.kind = AxisKind::EdgePair;
    cache.index_a = query.index_a as u8;
    cache.index_b = query.index_b as u8;
    manifold
}

/// Tries to rebuild the manifold from the axis of the previous step. Returns the manifold when
/// the cached feature still describes the pair.
fn try_cached_axis(
    hull_a: &Hull,
    hull_b: &Hull,
    b_to_a: &Pose,
    cache: &mut SatCache,
) -> Option<LocalManifold> {
    match cache.kind {
        AxisKind::Invalid => None,
        AxisKind::FaceA => {
            let plane = hull_a.planes()[cache.index_a as usize];
            let direction_in_b = -b_to_a.inv_rotate(plane.normal);
            let vertex = hull_b.support_vertex(direction_in_b);
            let support = b_to_a.transform_point(hull_b.points()[vertex]);
            let separation = plane.separation(support);
            if separation >= SPECULATIVE_DISTANCE {
                cache.hit = true;
                return Some(LocalManifold::default());
            }
            let mut local = SatCache::default();
            let manifold = build_face_a_contact(
                hull_a,
                hull_b,
                b_to_a,
                cache.index_a as usize,
                vertex,
                &mut local,
            );
            if manifold.count > 0 && (cache.separation - local.separation).abs() < LINEAR_SLOP {
                cache.hit = true;
                return Some(manifold);
            }
            None
        }
        AxisKind::FaceB => {
            let plane = hull_b.planes()[cache.index_b as usize];
            let direction_in_a = -(b_to_a.rotate(plane.normal));
            let vertex = hull_a.support_vertex(direction_in_a);
            let support = b_to_a.inv_transform_point(hull_a.points()[vertex]);
            let separation = plane.separation(support);
            if separation >= SPECULATIVE_DISTANCE {
                cache.hit = true;
                return Some(LocalManifold::default());
            }
            let mut local = SatCache::default();
            let manifold = build_face_b_contact(
                hull_a,
                hull_b,
                b_to_a,
                vertex,
                cache.index_b as usize,
                &mut local,
            );
            if manifold.count > 0 && (cache.separation - local.separation).abs() < LINEAR_SLOP {
                cache.hit = true;
                return Some(manifold);
            }
            None
        }
        AxisKind::EdgePair => try_cached_edge_pair(hull_a, hull_b, b_to_a, cache),
    }
}

/// The edge pair part of [`try_cached_axis`].
fn try_cached_edge_pair(
    hull_a: &Hull,
    hull_b: &Hull,
    b_to_a: &Pose,
    cache: &mut SatCache,
) -> Option<LocalManifold> {
    let index_a = cache.index_a as usize;
    let edge1 = hull_a.edges()[index_a];
    let twin1 = hull_a.edges()[index_a + 1];
    let p_a = hull_a.points()[edge1.origin];
    let q_a = hull_a.points()[twin1.origin];
    let e_a = q_a - p_a;
    let u_a = hull_a.planes()[edge1.face].normal;
    let v_a = hull_a.planes()[twin1.face].normal;
    let index_b = cache.index_b as usize;
    let edge2 = hull_b.edges()[index_b];
    let twin2 = hull_b.edges()[index_b + 1];
    let p_b = b_to_a.transform_point(hull_b.points()[edge2.origin]);
    let q_b = b_to_a.transform_point(hull_b.points()[twin2.origin]);
    let e_b = q_b - p_b;
    let u_b = b_to_a.rotate(hull_b.planes()[edge2.face].normal);
    let v_b = b_to_a.rotate(hull_b.planes()[twin2.face].normal);
    let cba = u_b.dot(e_a);
    let dba = v_b.dot(e_a);
    let adc = -u_a.dot(e_b);
    let bdc = -v_a.dot(e_b);
    if cba * dba < 0.0 && adc * bdc < 0.0 && cba * bdc > 0.0 {
        let squared_tolerance =
            crate::constants::PARALLEL_EDGE_TOL * crate::constants::PARALLEL_EDGE_TOL;
        if (cba * cba).max(dba * dba) >= squared_tolerance * e_a.length_squared() {
            let t = cba / (cba - dba);
            let axis = normalize(u_b + (v_b - u_b) * t);
            let separation = axis.dot(q_a - q_b);
            if separation > SPECULATIVE_DISTANCE {
                cache.hit = true;
                return Some(LocalManifold::default());
            }
            let query = SeparatingAxis {
                normal: -axis,
                separation: 0.0,
                index_a,
                index_b,
            };
            let mut local = SatCache::default();
            let manifold = build_edge_contact(hull_a, hull_b, b_to_a, &query, &mut local);
            if manifold.count > 0 && (cache.separation - local.separation).abs() < LINEAR_SLOP {
                cache.hit = true;
                return Some(manifold);
            }
        }
    }
    None
}

/// The manifold of two hulls, using and refreshing the cached separating axis.
pub fn collide_hulls(
    hull_a: &Hull,
    hull_b: &Hull,
    b_to_a: &Pose,
    cache: &mut SatCache,
) -> LocalManifold {
    cache.hit = false;
    if let Some(manifold) = try_cached_axis(hull_a, hull_b, b_to_a, cache) {
        return manifold;
    }
    *cache = SatCache::default();
    let query = compute_separating_axis(hull_a, hull_b, b_to_a, true);
    if query.separated != AxisKind::Invalid {
        let axis = match query.separated {
            AxisKind::FaceA => query.face_a,
            AxisKind::FaceB => query.face_b,
            _ => query.edge,
        };
        cache.kind = query.separated;
        cache.separation = axis.separation;
        cache.index_a = axis.index_a as u8;
        cache.index_b = axis.index_b as u8;
        return LocalManifold::default();
    }
    let mut manifold = if query.face_a.separation > query.face_b.separation {
        build_face_a_contact(
            hull_a,
            hull_b,
            b_to_a,
            query.face_a.index_a,
            query.face_a.index_b,
            cache,
        )
    } else {
        build_face_b_contact(
            hull_a,
            hull_b,
            b_to_a,
            query.face_b.index_a,
            query.face_b.index_b,
            cache,
        )
    };
    let edge_query = query.edge;
    if edge_query.index_a == NULL_INDEX {
        return manifold;
    }
    let face_separation = query.face_a.separation.max(query.face_b.separation);
    let clip_separation = cache.separation;
    if (manifold.count == 0 && edge_query.separation > face_separation)
        || edge_query.separation > clip_separation + LINEAR_SLOP
    {
        let mut edge_cache = SatCache::default();
        let edge_manifold =
            build_edge_contact(hull_a, hull_b, b_to_a, &edge_query, &mut edge_cache);
        if edge_manifold.count == 1 {
            manifold = edge_manifold;
            *cache = edge_cache;
        }
    }
    manifold
}
