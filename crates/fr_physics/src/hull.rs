//! Convex hulls in the half-edge representation the manifold code walks.
//!
//! Half-edges come in twin pairs stored at adjacent indices, so the edge `i` with `i` even has
//! its twin at `i + 1`. Faces are counter-clockwise seen from outside and every face has a unit
//! outward plane. [`Hull::cuboid`] builds a box and [`Hull::from_points`] a small general hull.

use std::collections::BTreeMap;

use fr_math::{Mat3, Vec3};

use crate::constants::MAX_HULL_ELEMENTS;
use crate::math::{Pose, normalize};
use fr_math::Aabb;
use fr_math::Plane;

/// One vertex of a hull.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HullVertex {
    /// A half-edge that starts at the vertex.
    pub edge: usize,
}

/// A directed edge of a hull face loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HalfEdge {
    /// The vertex the edge starts at.
    pub origin: usize,
    /// The edge that runs the other way along the same line.
    pub twin: usize,
    /// The face the edge belongs to.
    pub face: usize,
    /// The next edge counter-clockwise around the same face.
    pub next: usize,
}

/// One face of a hull.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HullFace {
    /// A half-edge of the face loop.
    pub edge: usize,
}

/// A convex polyhedron with its topology and mass properties for unit density.
#[derive(Clone, Debug)]
pub struct Hull {
    /// The vertex positions.
    points: Vec<Vec3>,
    /// One record per vertex.
    vertices: Vec<HullVertex>,
    /// The half-edges, twins at adjacent indices.
    edges: Vec<HalfEdge>,
    /// One record per face.
    faces: Vec<HullFace>,
    /// The outward plane of each face.
    planes: Vec<Plane>,
    /// The centre of mass.
    center: Vec3,
    /// The volume.
    volume: f32,
    /// The inertia about the centre of mass for unit density.
    central_inertia: Mat3,
    /// The radius of the largest sphere about the centre that fits inside.
    inner_radius: f32,
    /// The bounds of the vertices.
    aabb: Aabb,
}

/// A face loop of vertex indices while a hull is being built.
type FaceLoop = Vec<usize>;

impl Hull {
    /// A box of half sizes `half_extents` centred on the origin.
    pub fn cuboid(half_extents: Vec3) -> Self {
        let h = half_extents;
        let points = vec![
            Vec3::new(-h.x, -h.y, -h.z),
            Vec3::new(h.x, -h.y, -h.z),
            Vec3::new(h.x, h.y, -h.z),
            Vec3::new(-h.x, h.y, -h.z),
            Vec3::new(-h.x, -h.y, h.z),
            Vec3::new(h.x, -h.y, h.z),
            Vec3::new(h.x, h.y, h.z),
            Vec3::new(-h.x, h.y, h.z),
        ];
        let faces: Vec<FaceLoop> = vec![
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![2, 3, 7, 6],
            vec![0, 4, 7, 3],
            vec![1, 2, 6, 5],
        ];
        Self::from_faces(points, &faces).unwrap_or_else(Self::degenerate)
    }

    /// The convex hull of `points`, or `None` when they are flat, too many or not convex enough
    /// to stitch. Meant for small point sets of up to a hundred points.
    pub fn from_points(points: &[Vec3]) -> Option<Self> {
        let unique = dedupe(points);
        if unique.len() < 4 || unique.len() > MAX_HULL_ELEMENTS {
            return None;
        }
        let faces = hull_faces(&unique)?;
        let mut used = vec![usize::MAX; unique.len()];
        let mut kept = Vec::new();
        for face in &faces {
            for &vertex in face {
                if used[vertex] == usize::MAX {
                    used[vertex] = kept.len();
                    kept.push(unique[vertex]);
                }
            }
        }
        let remapped: Vec<FaceLoop> = faces
            .iter()
            .map(|face| face.iter().map(|&vertex| used[vertex]).collect())
            .collect();
        Self::from_faces(kept, &remapped)
    }

    /// A hull from vertex positions and counter-clockwise face loops; every edge must be shared by
    /// exactly two faces.
    fn from_faces(points: Vec<Vec3>, faces: &[FaceLoop]) -> Option<Self> {
        if points.len() > MAX_HULL_ELEMENTS || faces.len() > MAX_HULL_ELEMENTS {
            return None;
        }
        let mut edges: Vec<HalfEdge> = Vec::new();
        let mut slots: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        for (face_index, face) in faces.iter().enumerate() {
            for k in 0..face.len() {
                let (u, v) = (face[k], face[(k + 1) % face.len()]);
                let key = if u < v { (u, v) } else { (v, u) };
                let pair = *slots.entry(key).or_insert_with(|| {
                    let id = edges.len();
                    edges.push(HalfEdge {
                        origin: key.0,
                        twin: id + 1,
                        face: usize::MAX,
                        next: 0,
                    });
                    edges.push(HalfEdge {
                        origin: key.1,
                        twin: id,
                        face: usize::MAX,
                        next: 0,
                    });
                    id
                });
                let index = if u < v { pair } else { pair + 1 };
                if edges[index].face != usize::MAX {
                    return None;
                }
                edges[index].face = face_index;
            }
        }
        if edges.iter().any(|edge| edge.face == usize::MAX) {
            return None;
        }
        let directed = |u: usize, v: usize| -> Option<usize> {
            let key = if u < v { (u, v) } else { (v, u) };
            let pair = *slots.get(&key)?;
            Some(if u < v { pair } else { pair + 1 })
        };
        let mut face_records = Vec::with_capacity(faces.len());
        for face in faces {
            let n = face.len();
            face_records.push(HullFace {
                edge: directed(face[0], face[1 % n])?,
            });
            for k in 0..n {
                let current = directed(face[k], face[(k + 1) % n])?;
                let next = directed(face[(k + 1) % n], face[(k + 2) % n])?;
                edges[current].next = next;
            }
        }
        let mut vertices = vec![HullVertex { edge: usize::MAX }; points.len()];
        for (index, edge) in edges.iter().enumerate() {
            if vertices[edge.origin].edge == usize::MAX {
                vertices[edge.origin].edge = index;
            }
        }
        if vertices.iter().any(|vertex| vertex.edge == usize::MAX) {
            return None;
        }
        let planes = face_planes(&points, faces);
        let mut hull = Self {
            points,
            vertices,
            edges,
            faces: face_records,
            planes,
            center: Vec3::ZERO,
            volume: 0.0,
            central_inertia: Mat3::ZERO,
            inner_radius: 0.0,
            aabb: Aabb::default(),
        };
        hull.compute_properties();
        Some(hull)
    }

    /// A tiny box used if building a box ever failed.
    fn degenerate() -> Self {
        Self {
            points: vec![Vec3::ZERO],
            vertices: vec![HullVertex { edge: 0 }],
            edges: Vec::new(),
            faces: Vec::new(),
            planes: Vec::new(),
            center: Vec3::ZERO,
            volume: 0.0,
            central_inertia: Mat3::ZERO,
            inner_radius: 0.0,
            aabb: Aabb::default(),
        }
    }

    /// Computes the volume, centre of mass, inertia, inner radius and bounds from the faces.
    fn compute_properties(&mut self) {
        let count = self.points.len() as f32;
        let mut reference = Vec3::ZERO;
        for &point in &self.points {
            reference += point;
        }
        reference *= 1.0 / count;
        let mut volume = 0.0;
        let mut weighted = Vec3::ZERO;
        let mut covariance = Mat3::ZERO;
        for face_index in 0..self.faces.len() {
            let loop_points = self.face_points(face_index);
            let a = loop_points[0] - reference;
            for k in 1..loop_points.len() - 1 {
                let b = loop_points[k] - reference;
                let c = loop_points[k + 1] - reference;
                let tetra = a.dot(b.cross(c)) / 6.0;
                let sum = a + b + c;
                volume += tetra;
                weighted += sum * (0.25 * tetra);
                covariance +=
                    (outer(a, a) + outer(b, b) + outer(c, c) + outer(sum, sum)) * (tetra / 20.0);
            }
        }
        let offset = if volume > 0.0 {
            weighted * (1.0 / volume)
        } else {
            Vec3::ZERO
        };
        let central = covariance - outer(offset, offset) * volume;
        let trace = central.x_axis.x + central.y_axis.y + central.z_axis.z;
        self.volume = volume;
        self.center = reference + offset;
        self.central_inertia = Mat3::IDENTITY * trace - central;
        let mut inner = f32::MAX;
        for plane in &self.planes {
            inner = inner.min(-plane.separation(self.center));
        }
        self.inner_radius = if self.planes.is_empty() { 0.0 } else { inner };
        let mut lower = self.points[0];
        let mut upper = self.points[0];
        for &point in &self.points {
            lower = lower.min(point);
            upper = upper.max(point);
        }
        self.aabb = Aabb::new(lower, upper);
    }

    /// The points of the loop of face `face` in order.
    fn face_points(&self, face: usize) -> Vec<Vec3> {
        let start = self.faces[face].edge;
        let mut result = Vec::new();
        let mut edge = start;
        loop {
            result.push(self.points[self.edges[edge].origin]);
            edge = self.edges[edge].next;
            if edge == start {
                break;
            }
        }
        result
    }

    /// This hull with every point moved by `pose`.
    pub fn transformed(&self, pose: &Pose) -> Self {
        let mut hull = self.clone();
        for point in &mut hull.points {
            *point = pose.transform_point(*point);
        }
        for plane in &mut hull.planes {
            let normal = pose.rotate(plane.normal);
            *plane = Plane {
                normal,
                offset: plane.offset + normal.dot(pose.position),
            };
        }
        hull.compute_properties();
        hull
    }

    /// The vertex positions.
    pub fn points(&self) -> &[Vec3] {
        &self.points
    }

    /// The vertex records.
    pub fn vertices(&self) -> &[HullVertex] {
        &self.vertices
    }

    /// The half-edges.
    pub fn edges(&self) -> &[HalfEdge] {
        &self.edges
    }

    /// The face records.
    pub fn faces(&self) -> &[HullFace] {
        &self.faces
    }

    /// The outward plane of every face.
    pub fn planes(&self) -> &[Plane] {
        &self.planes
    }

    /// The centre of mass.
    pub fn center(&self) -> Vec3 {
        self.center
    }

    /// The volume.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// The inertia about the centre of mass for unit density.
    pub fn central_inertia(&self) -> Mat3 {
        self.central_inertia
    }

    /// The radius of the largest sphere about the centre of mass that fits inside.
    pub fn inner_radius(&self) -> f32 {
        self.inner_radius
    }

    /// The bounds of the vertices.
    pub fn aabb(&self) -> Aabb {
        self.aabb
    }

    /// The index of the vertex farthest along `direction`.
    pub fn support_vertex(&self, direction: Vec3) -> usize {
        let mut best = 0;
        let mut best_dot = f32::MIN;
        for (index, point) in self.points.iter().enumerate() {
            let dot = direction.x * point.x + direction.y * point.y + direction.z * point.z;
            if dot > best_dot {
                best = index;
                best_dot = dot;
            }
        }
        best
    }

    /// The index of the face whose normal points most along `direction`.
    pub fn support_face(&self, direction: Vec3) -> usize {
        let mut best = 0;
        let mut best_dot = f32::MIN;
        for (index, plane) in self.planes.iter().enumerate() {
            let dot = plane.normal.dot(direction);
            if dot > best_dot {
                best = index;
                best_dot = dot;
            }
        }
        best
    }

    /// The bounds of the hull moved by `pose`.
    pub fn compute_aabb(&self, pose: &Pose) -> Aabb {
        let first = pose.transform_point(self.points[0]);
        let mut lower = first;
        let mut upper = first;
        for &point in &self.points[1..] {
            let p = pose.transform_point(point);
            lower = lower.min(p);
            upper = upper.max(p);
        }
        Aabb::new(lower, upper)
    }

    /// Casts a segment against the hull in its local frame. Returns the entry fraction and the
    /// face normal; a segment that starts inside hits at fraction zero with a zero normal.
    pub fn ray_cast(
        &self,
        origin: Vec3,
        translation: Vec3,
        max_fraction: f32,
    ) -> Option<(f32, Vec3)> {
        let mut lower = 0.0_f32;
        let mut upper = max_fraction;
        let mut index = usize::MAX;
        for (i, plane) in self.planes.iter().enumerate() {
            let distance = plane.separation(origin);
            let denominator = plane.normal.dot(translation);
            if denominator == 0.0 {
                if distance > 0.0 {
                    return None;
                }
                continue;
            }
            let t = -distance / denominator;
            if denominator < 0.0 {
                if t > lower {
                    lower = t;
                    index = i;
                }
            } else if t < upper {
                upper = t;
            }
            if upper < lower {
                return None;
            }
        }
        if index == usize::MAX {
            return Some((0.0, Vec3::ZERO));
        }
        Some((lower, self.planes[index].normal))
    }
}

/// The outer product `a * transpose(b)`.
fn outer(a: Vec3, b: Vec3) -> Mat3 {
    Mat3::from_cols(a * b.x, a * b.y, a * b.z)
}

/// The points of `points` without exact repeats, in first-seen order.
fn dedupe(points: &[Vec3]) -> Vec<Vec3> {
    let mut unique: Vec<Vec3> = Vec::new();
    for &point in points {
        if !unique
            .iter()
            .any(|&other| (other - point).length_squared() < 1e-10)
        {
            unique.push(point);
        }
    }
    unique
}

/// The outward unit plane of every face loop by Newell's method.
fn face_planes(points: &[Vec3], faces: &[FaceLoop]) -> Vec<Plane> {
    faces
        .iter()
        .map(|face| {
            let mut normal = Vec3::ZERO;
            let mut centroid = Vec3::ZERO;
            for k in 0..face.len() {
                let a = points[face[k]];
                let b = points[face[(k + 1) % face.len()]];
                normal.x += (a.y - b.y) * (a.z + b.z);
                normal.y += (a.z - b.z) * (a.x + b.x);
                normal.z += (a.x - b.x) * (a.y + b.y);
                centroid += a;
            }
            let normal = normalize(normal);
            Plane::from_normal_and_point(normal, centroid * (1.0 / face.len() as f32))
        })
        .collect()
}

/// The faces of the convex hull of `points` as counter-clockwise loops, found by testing every
/// supporting plane through three points.
fn hull_faces(points: &[Vec3]) -> Option<Vec<FaceLoop>> {
    let tolerance = 1e-4;
    let mut inside = Vec3::ZERO;
    for &point in points {
        inside += point;
    }
    inside *= 1.0 / points.len() as f32;
    let mut sets: Vec<Vec<usize>> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            for k in j + 1..points.len() {
                let mut normal = normalize((points[j] - points[i]).cross(points[k] - points[i]));
                if normal == Vec3::ZERO {
                    continue;
                }
                if normal.dot(inside - points[i]) > 0.0 {
                    normal = -normal;
                }
                let offset = normal.dot(points[i]);
                let mut on_plane = Vec::new();
                let mut supporting = true;
                for (index, point) in points.iter().enumerate() {
                    let distance = normal.dot(*point) - offset;
                    if distance > tolerance {
                        supporting = false;
                        break;
                    }
                    if distance >= -tolerance {
                        on_plane.push(index);
                    }
                }
                if supporting && !sets.contains(&on_plane) {
                    sets.push(on_plane);
                    normals.push(normal);
                }
            }
        }
    }
    if sets.len() < 4 {
        return None;
    }
    let mut faces = Vec::new();
    for (set, normal) in sets.iter().zip(&normals) {
        faces.push(convex_loop(points, set, *normal)?);
    }
    Some(faces)
}

/// The corners of the convex polygon of the coplanar points `set` as a counter-clockwise loop
/// seen against `normal`, without collinear vertices.
fn convex_loop(points: &[Vec3], set: &[usize], normal: Vec3) -> Option<FaceLoop> {
    let u = crate::math::perp(normal);
    let v = normal.cross(u);
    let mut projected: Vec<(f32, f32, usize)> = set
        .iter()
        .map(|&index| (points[index].dot(u), points[index].dot(v), index))
        .collect();
    projected.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let cross = |o: &(f32, f32, usize), a: &(f32, f32, usize), b: &(f32, f32, usize)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut chain: Vec<(f32, f32, usize)> = Vec::new();
    for point in &projected {
        while chain.len() >= 2
            && cross(&chain[chain.len() - 2], &chain[chain.len() - 1], point) <= 1e-7
        {
            chain.pop();
        }
        chain.push(*point);
    }
    let lower_len = chain.len() + 1;
    for point in projected.iter().rev().skip(1) {
        while chain.len() >= lower_len
            && cross(&chain[chain.len() - 2], &chain[chain.len() - 1], point) <= 1e-7
        {
            chain.pop();
        }
        chain.push(*point);
    }
    chain.pop();
    if chain.len() < 3 {
        return None;
    }
    Some(chain.iter().map(|point| point.2).collect())
}
