//! The GJK distance between convex point clouds, and distances between segments and lines.

use fr_core::Vec3;

use crate::geometry::ShapeProxy;
use crate::math::{Pose, mul_add, normalize};

/// The most GJK iterations before the search gives up.
const MAX_GJK_ITERATIONS: usize = 32;

/// The closest points between two segments or lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentDistance {
    /// The closest point on the first segment.
    pub point1: Vec3,
    /// The fraction along the first segment of `point1`.
    pub fraction1: f32,
    /// The closest point on the second segment.
    pub point2: Vec3,
    /// The fraction along the second segment of `point2`.
    pub fraction2: f32,
}

impl SegmentDistance {
    /// Whether both closest points lie within their segments.
    pub fn is_within_segments(&self) -> bool {
        (0.0..=1.0).contains(&self.fraction1) && (0.0..=1.0).contains(&self.fraction2)
    }
}

/// The closest points between the infinite lines `p1 + s * d1` and `p2 + t * d2`.
pub fn line_distance(p1: Vec3, d1: Vec3, p2: Vec3, d2: Vec3) -> SegmentDistance {
    let a11 = d1.dot(d1);
    let a12 = -d1.dot(d2);
    let a21 = d2.dot(d1);
    let a22 = -d2.dot(d2);
    let w = p1 - p2;
    let b1 = -d1.dot(w);
    let b2 = -d2.dot(w);
    let det = a11 * a22 - a12 * a21;
    if det * det < 1000.0 * f32::MIN_POSITIVE {
        let s1 = (p2 - p1).dot(d1) / d1.dot(d1);
        return SegmentDistance {
            point1: mul_add(p1, s1, d1),
            fraction1: s1,
            point2: p2,
            fraction2: 0.0,
        };
    }
    let s1 = (a22 * b1 - a12 * b2) / det;
    let s2 = (a11 * b2 - a21 * b1) / det;
    SegmentDistance {
        point1: mul_add(p1, s1, d1),
        fraction1: s1,
        point2: mul_add(p2, s2, d2),
        fraction2: s2,
    }
}

/// The closest points between the segments `p1`-`q1` and `p2`-`q2`.
pub fn segment_distance(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> SegmentDistance {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let b = d1.dot(d2);
    let c = d1.dot(r);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let tiny = 100.0 * f32::EPSILON;
    if a < tiny && e < tiny {
        return SegmentDistance {
            point1: p1,
            fraction1: 0.0,
            point2: p2,
            fraction2: 0.0,
        };
    }
    if a < tiny {
        let s2 = (f / e).clamp(0.0, 1.0);
        return SegmentDistance {
            point1: p1,
            fraction1: 0.0,
            point2: mul_add(p2, s2, d2),
            fraction2: s2,
        };
    }
    if e < tiny {
        let s1 = (-c / a).clamp(0.0, 1.0);
        return SegmentDistance {
            point1: mul_add(p1, s1, d1),
            fraction1: s1,
            point2: p2,
            fraction2: 0.0,
        };
    }
    let denom = a * e - b * b;
    let mut s1 = if denom > 1000.0 * f32::MIN_POSITIVE {
        ((b * f - c * e) / denom).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut s2 = (b * s1 + f) / e;
    if s2 < 0.0 {
        s1 = (-c / a).clamp(0.0, 1.0);
        s2 = 0.0;
    } else if s2 > 1.0 {
        s1 = ((b - c) / a).clamp(0.0, 1.0);
        s2 = 1.0;
    }
    SegmentDistance {
        point1: mul_add(p1, s1, d1),
        fraction1: s1,
        point2: mul_add(p2, s2, d2),
        fraction2: s2,
    }
}

/// The point of the segment `a`-`b` closest to `q`.
pub fn point_to_segment(a: Vec3, b: Vec3, q: Vec3) -> Vec3 {
    let ab = b - a;
    let aq = q - a;
    let alpha = ab.dot(aq);
    if alpha <= 0.0 {
        return a;
    }
    let denominator = ab.dot(ab);
    if alpha > denominator {
        return b;
    }
    mul_add(a, alpha / denominator, ab)
}

/// The index of the point farthest along `axis`.
pub fn proxy_support(points: &[Vec3], axis: Vec3) -> usize {
    let origin = points[0];
    let mut max_index = 0;
    let mut max_projection = 0.0;
    for (index, point) in points.iter().enumerate().skip(1) {
        let projection = axis.dot(*point - origin);
        if projection > max_projection {
            max_index = index;
            max_projection = projection;
        }
    }
    max_index
}

/// The simplex of a finished GJK query, kept to start the next query from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimplexCache {
    /// The size of the simplex, zero when invalid.
    pub count: u16,
    /// The support index on shape A of each vertex.
    pub index_a: [u8; 4],
    /// The support index on shape B of each vertex.
    pub index_b: [u8; 4],
    /// The size of the simplex when it was stored, used to detect a stale cache.
    pub metric: f32,
}

/// The two proxies of a distance query and the pose of B in the frame of A.
#[derive(Clone, Copy, Debug)]
pub struct DistanceInput<'a> {
    /// The first shape, in its own frame.
    pub proxy_a: ShapeProxy<'a>,
    /// The second shape, in its own frame.
    pub proxy_b: ShapeProxy<'a>,
    /// The pose of B in the frame of A.
    pub transform: Pose,
    /// Whether the radii of the proxies shrink the distance.
    pub use_radii: bool,
}

/// The result of a distance query, in the frame of A.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DistanceOutput {
    /// The closest point on A.
    pub point_a: Vec3,
    /// The closest point on B.
    pub point_b: Vec3,
    /// The unit direction from A to B, zero when overlapped.
    pub normal: Vec3,
    /// The distance, zero when overlapped.
    pub distance: f32,
    /// The number of iterations used.
    pub iterations: usize,
}

/// One vertex of the Minkowski difference simplex.
#[derive(Clone, Copy, Debug, Default)]
struct SimplexVertex {
    /// The support point on A.
    w_a: Vec3,
    /// The support point on B.
    w_b: Vec3,
    /// `w_b - w_a`.
    w: Vec3,
    /// The barycentric weight of the vertex.
    a: f32,
    /// The index of the support point on A.
    index_a: usize,
    /// The index of the support point on B.
    index_b: usize,
}

/// A simplex of up to four vertices.
#[derive(Clone, Copy, Debug, Default)]
struct Simplex {
    /// The vertices, of which `count` are used.
    vertices: [SimplexVertex; 4],
    /// The number of vertices in use.
    count: usize,
}

/// The edge barycentric numerators of `a`-`b` and their divisor.
fn barycentric_edge(a: Vec3, b: Vec3) -> [f32; 3] {
    let ab = b - a;
    [b.dot(ab), -a.dot(ab), ab.dot(ab)]
}

/// The triangle barycentric numerators of `a`-`b`-`c` and their divisor.
fn barycentric_tri(a: Vec3, b: Vec3, c: Vec3) -> [f32; 4] {
    let ab = b - a;
    let ac = c - a;
    let b_x_c = b.cross(c);
    let c_x_a = c.cross(a);
    let a_x_b = a.cross(b);
    let ab_x_ac = ab.cross(ac);
    [
        b_x_c.dot(ab_x_ac),
        c_x_a.dot(ab_x_ac),
        a_x_b.dot(ab_x_ac),
        ab_x_ac.dot(ab_x_ac),
    ]
}

/// The scalar triple product `a . (b x c)` computed component by component.
fn triple(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let dx = b.y * c.z - b.z * c.y;
    let dy = b.z * c.x - b.x * c.z;
    let dz = b.x * c.y - b.y * c.x;
    a.x * dx + a.y * dy + a.z * dz
}

/// The tetrahedron barycentric numerators of `a`-`b`-`c`-`d` and their positive divisor.
fn barycentric_tet(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> [f32; 5] {
    let divisor = triple(b - a, c - a, d - a);
    let sign = if divisor < 0.0 { -1.0 } else { 1.0 };
    [
        sign * triple(b, c, d),
        sign * triple(a, d, c),
        sign * triple(a, b, d),
        sign * triple(a, c, b),
        sign * divisor,
    ]
}

impl Simplex {
    /// The size of the simplex, for deciding whether a cached one is still useful.
    fn metric(&self) -> f32 {
        let v = &self.vertices;
        match self.count {
            2 => (v[0].w - v[1].w).length(),
            3 => (v[1].w - v[0].w).cross(v[2].w - v[0].w).length() / 2.0,
            4 => triple(v[1].w - v[0].w, v[2].w - v[0].w, v[3].w - v[0].w) / 6.0,
            _ => 0.0,
        }
    }

    /// Writes the simplex into `cache`.
    fn write_cache(&self, cache: &mut SimplexCache) {
        cache.metric = self.metric();
        cache.count = self.count as u16;
        for index in 0..self.count {
            cache.index_a[index] = self.vertices[index].index_a as u8;
            cache.index_b[index] = self.vertices[index].index_b as u8;
        }
    }

    /// Keeps the listed vertices with the weights `numerators / divisor`.
    fn reduce(&mut self, keep: &[usize], numerators: &[f32], divisor: f32) -> bool {
        if divisor <= 0.0 {
            return false;
        }
        let old = self.vertices;
        self.count = keep.len();
        for (slot, (&source, &numerator)) in keep.iter().zip(numerators).enumerate() {
            self.vertices[slot] = old[source];
            self.vertices[slot].a = numerator / divisor;
        }
        true
    }

    /// Keeps only vertex `index` with full weight.
    fn reduce_to_vertex(&mut self, index: usize) -> bool {
        self.vertices[0] = self.vertices[index];
        self.vertices[0].a = 1.0;
        self.count = 1;
        true
    }

    /// Reduces a two vertex simplex to the feature closest to the origin.
    fn solve2(&mut self) -> bool {
        let a = self.vertices[0].w;
        let b = self.vertices[1].w;
        let ab = b - a;
        let divisor = ab.dot(ab);
        let u = b.dot(ab);
        let v = -a.dot(ab);
        if v <= 0.0 {
            return self.reduce_to_vertex(0);
        }
        if u <= 0.0 {
            return self.reduce_to_vertex(1);
        }
        if divisor <= 0.0 {
            return false;
        }
        let denominator = 1.0 / divisor;
        self.vertices[0].a = denominator * u;
        self.vertices[1].a = denominator * v;
        true
    }

    /// Reduces a three vertex simplex to the feature closest to the origin.
    fn solve3(&mut self) -> bool {
        let w1 = self.vertices[0].w;
        let w2 = self.vertices[1].w;
        let w3 = self.vertices[2].w;
        let w_ab = barycentric_edge(w1, w2);
        let w_bc = barycentric_edge(w2, w3);
        let w_ca = barycentric_edge(w3, w1);
        if w_ab[1] <= 0.0 && w_ca[0] <= 0.0 {
            return self.reduce_to_vertex(0);
        }
        if w_bc[1] <= 0.0 && w_ab[0] <= 0.0 {
            return self.reduce_to_vertex(1);
        }
        if w_ca[1] <= 0.0 && w_bc[0] <= 0.0 {
            return self.reduce_to_vertex(2);
        }
        let w_abc = barycentric_tri(w1, w2, w3);
        if w_abc[2] <= 0.0 && w_ab[0] > 0.0 && w_ab[1] > 0.0 {
            return self.reduce(&[0, 1], &w_ab[..2], w_ab[2]);
        }
        if w_abc[0] <= 0.0 && w_bc[0] > 0.0 && w_bc[1] > 0.0 {
            return self.reduce(&[1, 2], &w_bc[..2], w_bc[2]);
        }
        if w_abc[1] <= 0.0 && w_ca[0] > 0.0 && w_ca[1] > 0.0 {
            return self.reduce(&[2, 0], &w_ca[..2], w_ca[2]);
        }
        self.reduce(&[0, 1, 2], &w_abc[..3], w_abc[3])
    }

    /// Reduces a four vertex simplex to the feature closest to the origin.
    fn solve4(&mut self) -> bool {
        let va = self.vertices[0].w;
        let vb = self.vertices[1].w;
        let vc = self.vertices[2].w;
        let vd = self.vertices[3].w;
        let w_ab = barycentric_edge(va, vb);
        let w_ac = barycentric_edge(va, vc);
        let w_ad = barycentric_edge(va, vd);
        let w_bc = barycentric_edge(vb, vc);
        let w_cd = barycentric_edge(vc, vd);
        let w_db = barycentric_edge(vd, vb);
        if w_ab[1] <= 0.0 && w_ac[1] <= 0.0 && w_ad[1] <= 0.0 {
            return self.reduce_to_vertex(0);
        }
        if w_ab[0] <= 0.0 && w_db[0] <= 0.0 && w_bc[1] <= 0.0 {
            return self.reduce_to_vertex(1);
        }
        if w_ac[0] <= 0.0 && w_bc[0] <= 0.0 && w_cd[1] <= 0.0 {
            return self.reduce_to_vertex(2);
        }
        if w_ad[0] <= 0.0 && w_cd[0] <= 0.0 && w_db[1] <= 0.0 {
            return self.reduce_to_vertex(3);
        }
        let w_acb = barycentric_tri(va, vc, vb);
        let w_abd = barycentric_tri(va, vb, vd);
        let w_adc = barycentric_tri(va, vd, vc);
        let w_bcd = barycentric_tri(vb, vc, vd);
        if w_abd[2] <= 0.0 && w_acb[1] <= 0.0 && w_ab[0] > 0.0 && w_ab[1] > 0.0 {
            return self.reduce(&[0, 1], &w_ab[..2], w_ab[2]);
        }
        if w_acb[2] <= 0.0 && w_adc[1] <= 0.0 && w_ac[0] > 0.0 && w_ac[1] > 0.0 {
            return self.reduce(&[0, 2], &w_ac[..2], w_ac[2]);
        }
        if w_adc[2] <= 0.0 && w_abd[1] <= 0.0 && w_ad[0] > 0.0 && w_ad[1] > 0.0 {
            return self.reduce(&[0, 3], &w_ad[..2], w_ad[2]);
        }
        if w_acb[0] <= 0.0 && w_bcd[2] <= 0.0 && w_bc[0] > 0.0 && w_bc[1] > 0.0 {
            return self.reduce(&[1, 2], &w_bc[..2], w_bc[2]);
        }
        if w_adc[0] <= 0.0 && w_bcd[0] <= 0.0 && w_cd[0] > 0.0 && w_cd[1] > 0.0 {
            return self.reduce(&[2, 3], &w_cd[..2], w_cd[2]);
        }
        if w_abd[0] <= 0.0 && w_bcd[1] <= 0.0 && w_db[0] > 0.0 && w_db[1] > 0.0 {
            return self.reduce(&[3, 1], &w_db[..2], w_db[2]);
        }
        let w_abcd = barycentric_tet(va, vb, vc, vd);
        if w_abcd[3] < 0.0 && w_acb[0] > 0.0 && w_acb[1] > 0.0 && w_acb[2] > 0.0 {
            return self.reduce(&[0, 2, 1], &w_acb[..3], w_acb[3]);
        }
        if w_abcd[2] < 0.0 && w_abd[0] > 0.0 && w_abd[1] > 0.0 && w_abd[2] > 0.0 {
            return self.reduce(&[0, 1, 3], &w_abd[..3], w_abd[3]);
        }
        if w_abcd[1] < 0.0 && w_adc[0] > 0.0 && w_adc[1] > 0.0 && w_adc[2] > 0.0 {
            return self.reduce(&[0, 3, 2], &w_adc[..3], w_adc[3]);
        }
        if w_abcd[0] < 0.0 && w_bcd[0] > 0.0 && w_bcd[1] > 0.0 && w_bcd[2] > 0.0 {
            return self.reduce(&[1, 2, 3], &w_bcd[..3], w_bcd[3]);
        }
        self.reduce(&[0, 1, 2, 3], &w_abcd[..4], w_abcd[4])
    }

    /// The weighted closest point of the Minkowski difference.
    fn closest_point(&self) -> Vec3 {
        let v = &self.vertices;
        match self.count {
            1 => v[0].w,
            2 => blend2(v[0].a, v[0].w, v[1].a, v[1].w),
            3 => blend3(v[0].a, v[0].w, v[1].a, v[1].w, v[2].a, v[2].w),
            _ => blend2(v[0].a, v[0].w, v[1].a, v[1].w) + blend2(v[2].a, v[2].w, v[3].a, v[3].w),
        }
    }

    /// The witness points on A and B.
    fn witness_points(&self) -> (Vec3, Vec3) {
        let v = &self.vertices;
        match self.count {
            1 => (v[0].w_a, v[0].w_b),
            2 => (
                blend2(v[0].a, v[0].w_a, v[1].a, v[1].w_a),
                blend2(v[0].a, v[0].w_b, v[1].a, v[1].w_b),
            ),
            3 => (
                blend3(v[0].a, v[0].w_a, v[1].a, v[1].w_a, v[2].a, v[2].w_a),
                blend3(v[0].a, v[0].w_b, v[1].a, v[1].w_b, v[2].a, v[2].w_b),
            ),
            _ => {
                let sum = blend2(v[0].a, v[0].w_a, v[1].a, v[1].w_a)
                    + blend2(v[2].a, v[2].w_a, v[3].a, v[3].w_a);
                (sum, sum)
            }
        }
    }
}

/// `s * a + t * b`.
fn blend2(s: f32, a: Vec3, t: f32, b: Vec3) -> Vec3 {
    Vec3::new(s * a.x + t * b.x, s * a.y + t * b.y, s * a.z + t * b.z)
}

/// `s * a + t * b + u * c`.
fn blend3(s: f32, a: Vec3, t: f32, b: Vec3, u: f32, c: Vec3) -> Vec3 {
    Vec3::new(
        s * a.x + t * b.x + u * c.x,
        s * a.y + t * b.y + u * c.y,
        s * a.z + t * b.z + u * c.z,
    )
}

/// The support vertex of the Minkowski difference along `search_direction`.
fn support_vertex(input: &DistanceInput<'_>, search_direction: Vec3) -> SimplexVertex {
    let index_a = proxy_support(input.proxy_a.points, -search_direction);
    let support_a = input.proxy_a.points[index_a];
    let direction_b = input.transform.inv_rotate(search_direction);
    let index_b = proxy_support(input.proxy_b.points, direction_b);
    let support_b = input
        .transform
        .transform_point(input.proxy_b.points[index_b]);
    SimplexVertex {
        w_a: support_a,
        w_b: support_b,
        w: support_b - support_a,
        a: 0.0,
        index_a,
        index_b,
    }
}

/// The initial simplex of a query, from the cache when it still describes the shapes.
fn initial_simplex(input: &DistanceInput<'_>, cache: &SimplexCache) -> Simplex {
    let mut simplex = Simplex {
        count: cache.count as usize,
        ..Simplex::default()
    };
    for i in 0..simplex.count {
        let index_a = cache.index_a[i] as usize;
        let index_b = cache.index_b[i] as usize;
        let vertex_a = input.proxy_a.points[index_a];
        let vertex_b = input
            .transform
            .transform_point(input.proxy_b.points[index_b]);
        simplex.vertices[i] = SimplexVertex {
            w_a: vertex_a,
            w_b: vertex_b,
            w: vertex_b - vertex_a,
            a: 0.0,
            index_a,
            index_b,
        };
    }
    if simplex.count > 0 {
        let metric1 = cache.metric;
        let metric2 = simplex.metric();
        if 2.0 * metric1 < metric2 || metric2 < 0.5 * metric1 || metric2 < f32::EPSILON {
            simplex.count = 0;
        }
    }
    if simplex.count == 0 {
        let vertex_a = input.proxy_a.points[0];
        let vertex_b = input.transform.transform_point(input.proxy_b.points[0]);
        simplex.count = 1;
        simplex.vertices[0] = SimplexVertex {
            w_a: vertex_a,
            w_b: vertex_b,
            w: vertex_b - vertex_a,
            a: 0.0,
            index_a: 0,
            index_b: 0,
        };
    }
    simplex
}

/// The GJK search direction that moves the simplex towards the origin.
fn search_direction(simplex: &Simplex) -> Vec3 {
    let v = &simplex.vertices;
    match simplex.count {
        1 => -v[0].w,
        2 => {
            let a = v[0].w;
            let ab = v[1].w - a;
            (ab.cross(-a)).cross(ab)
        }
        _ => {
            let a = v[0].w;
            let ab = v[1].w - a;
            let ac = v[2].w - a;
            let n = ab.cross(ac);
            if n.dot(a) < 0.0 { n } else { -n }
        }
    }
}

/// The distance between two convex shapes in the frame of A, starting from `cache` and updating
/// it when the simplex is useful for the next query.
pub fn shape_distance(input: &DistanceInput<'_>, cache: &mut SimplexCache) -> DistanceOutput {
    let mut simplex = initial_simplex(input, cache);
    let mut backup = Simplex::default();
    let mut output = DistanceOutput::default();
    let mut distance_squared = f32::MAX;
    let mut normal = Vec3::ZERO;
    let mut iteration = 0;
    while iteration < MAX_GJK_ITERATIONS {
        let solved = match simplex.count {
            1 => {
                simplex.vertices[0].a = 1.0;
                true
            }
            2 => simplex.solve2(),
            3 => simplex.solve3(),
            _ => simplex.solve4(),
        };
        if !solved {
            simplex = backup;
            break;
        }
        if simplex.count == 4 {
            let (point_a, point_b) = simplex.witness_points();
            output.point_a = point_a;
            output.point_b = point_b;
            output.iterations = iteration;
            return output;
        }
        let old_distance_squared = distance_squared;
        let closest = simplex.closest_point();
        distance_squared = closest.dot(closest);
        if distance_squared >= old_distance_squared {
            simplex = backup;
            break;
        }
        let direction = search_direction(&simplex);
        if direction.length_squared() < 1000.0 * f32::MIN_POSITIVE {
            let (point_a, point_b) = simplex.witness_points();
            output.point_a = point_a;
            output.point_b = point_b;
            output.iterations = iteration;
            return output;
        }
        normal = -direction;
        let vertex = support_vertex(input, direction);
        backup = simplex;
        let duplicate = simplex.vertices[..simplex.count]
            .iter()
            .any(|v| v.index_a == vertex.index_a && v.index_b == vertex.index_b);
        if duplicate {
            break;
        }
        simplex.vertices[simplex.count] = vertex;
        simplex.count += 1;
        iteration += 1;
    }
    let (point_a, point_b) = simplex.witness_points();
    output.point_a = point_a;
    output.point_b = point_b;
    output.iterations = iteration;
    let normal = normalize(normal);
    if (normal.length_squared() - 1.0).abs() > 1e-4 {
        return output;
    }
    output.distance = (point_a - point_b).length();
    output.normal = normal;
    if input.use_radii {
        let r_a = input.proxy_a.radius;
        let r_b = input.proxy_b.radius;
        output.distance = (output.distance - r_a - r_b).max(0.0);
        output.point_a = mul_add(output.point_a, r_a, normal);
        output.point_b = mul_add(output.point_b, -r_b, normal);
    }
    simplex.write_cache(cache);
    output
}
