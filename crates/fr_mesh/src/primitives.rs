//! Built-in shapes: a cube, a sphere, a plane, a cylinder and a capsule.

use std::f32::consts::{PI, TAU};

use fr_math::{Vec2, Vec3};

use crate::mesh::MeshData;

/// Appends one quad with corners `a`, `b`, `c`, `d` in counter-clockwise order
/// seen from the front, whose texture coordinates run over `uv_extent`.
fn push_quad(mesh: &mut MeshData, corners: [Vec3; 4], normal: Vec3, uv_extent: Vec2) {
    let base = mesh.positions.len() as u32;
    let uvs = [
        Vec2::new(0.0, uv_extent.y),
        Vec2::new(uv_extent.x, uv_extent.y),
        Vec2::new(uv_extent.x, 0.0),
        Vec2::new(0.0, 0.0),
    ];
    mesh.positions.extend(corners);
    mesh.normals.extend([normal; 4]);
    mesh.uvs.extend(uvs);
    mesh.indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// A mesh with its tangents derived from the geometry just built.
fn finish(mut mesh: MeshData) -> MeshData {
    mesh.compute_tangents();
    mesh
}

/// A cube centred on the origin whose edges are `size` long.
pub fn cube(size: f32) -> MeshData {
    let h = size * 0.5;
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::NEG_X),
    ];
    let mut mesh = MeshData::default();
    for (normal, up, right) in faces {
        let center = normal * h;
        let corners = [
            center - right * h - up * h,
            center + right * h - up * h,
            center + right * h + up * h,
            center - right * h + up * h,
        ];
        push_quad(&mut mesh, corners, normal, Vec2::ONE);
    }
    finish(mesh)
}

/// A flat rectangle in the XZ plane facing up, centred on the origin.
///
/// `size` is its extent along X and Z; its texture coordinates run from zero to
/// `uv_repeat` along each axis, so a repeating texture tiles that many times.
pub fn plane(size: Vec2, uv_repeat: f32) -> MeshData {
    let half = size * 0.5;
    let mut mesh = MeshData::default();
    let corners = [
        Vec3::new(-half.x, 0.0, half.y),
        Vec3::new(half.x, 0.0, half.y),
        Vec3::new(half.x, 0.0, -half.y),
        Vec3::new(-half.x, 0.0, -half.y),
    ];
    push_quad(&mut mesh, corners, Vec3::Y, Vec2::splat(uv_repeat));
    finish(mesh)
}

/// A UV sphere centred on the origin with `segments` slices around Y and
/// `rings` bands from pole to pole, each clamped to at least 3 and 2.
pub fn sphere(radius: f32, segments: u32, rings: u32) -> MeshData {
    let segments = segments.max(3);
    let rings = rings.max(2);
    let mut mesh = MeshData::default();
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let polar = v * PI;
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let azimuth = u * TAU;
            let normal = Vec3::new(
                polar.sin() * azimuth.cos(),
                polar.cos(),
                -polar.sin() * azimuth.sin(),
            );
            mesh.positions.push(normal * radius);
            mesh.normals.push(normal);
            mesh.uvs.push(Vec2::new(u, v));
        }
    }
    let stride = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * stride + segment;
            let b = a + stride;
            mesh.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    finish(mesh)
}

/// One point of the outline a surface of revolution is swept from.
struct ProfilePoint {
    /// Distance from the Y axis and height.
    position: Vec2,
    /// The surface normal there as a component away from the axis and one along Y.
    normal: Vec2,
}

impl ProfilePoint {
    /// A point at `radius` and `y` with the normal `(normal_radial, normal_y)`.
    fn new(radius: f32, y: f32, normal_radial: f32, normal_y: f32) -> Self {
        Self {
            position: Vec2::new(radius, y),
            normal: Vec2::new(normal_radial, normal_y),
        }
    }
}

/// Sweeps `profile`, listed from top to bottom, once around Y in `segments`
/// slices and appends the surface.
///
/// The texture coordinate `u` runs around the axis and `v` along the profile by
/// its length.
fn push_revolved(mesh: &mut MeshData, profile: &[ProfilePoint], segments: u32) {
    let base = mesh.positions.len() as u32;
    let lengths: Vec<f32> = profile
        .iter()
        .scan((0.0, None::<Vec2>), |(total, last), point| {
            if let Some(previous) = last.replace(point.position) {
                *total += previous.distance(point.position);
            }
            Some(*total)
        })
        .collect();
    let total = lengths.last().copied().unwrap_or(0.0);
    for (point, length) in profile.iter().zip(&lengths) {
        let v = if total > 0.0 { length / total } else { 0.0 };
        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let azimuth = u * TAU;
            let outward = Vec3::new(azimuth.cos(), 0.0, -azimuth.sin());
            mesh.positions
                .push(outward * point.position.x + Vec3::Y * point.position.y);
            mesh.normals
                .push(outward * point.normal.x + Vec3::Y * point.normal.y);
            mesh.uvs.push(Vec2::new(u, v));
        }
    }
    let stride = segments + 1;
    for row in 0..profile.len().saturating_sub(1) as u32 {
        for segment in 0..segments {
            let a = base + row * stride + segment;
            let b = a + stride;
            mesh.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
}

/// A closed cylinder around Y centred on the origin, with `segments` slices
/// clamped to at least 3.
///
/// The side has smooth normals; each cap is flat with its own vertices.
pub fn cylinder(radius: f32, height: f32, segments: u32) -> MeshData {
    let segments = segments.max(3);
    let half = height * 0.5;
    let mut mesh = MeshData::default();
    let top = [
        ProfilePoint::new(0.0, half, 0.0, 1.0),
        ProfilePoint::new(radius, half, 0.0, 1.0),
    ];
    let side = [
        ProfilePoint::new(radius, half, 1.0, 0.0),
        ProfilePoint::new(radius, -half, 1.0, 0.0),
    ];
    let bottom = [
        ProfilePoint::new(radius, -half, 0.0, -1.0),
        ProfilePoint::new(0.0, -half, 0.0, -1.0),
    ];
    for profile in [&top[..], &side[..], &bottom[..]] {
        push_revolved(&mut mesh, profile, segments);
    }
    finish(mesh)
}

/// A capsule around Y centred on the origin: a cylinder of `cylinder_height`
/// capped by two hemispheres of `radius`, so it is `cylinder_height + 2 * radius` long.
///
/// `segments` slices around Y and `rings` bands per hemisphere are clamped to at
/// least 3 and 2.
pub fn capsule(radius: f32, cylinder_height: f32, segments: u32, rings: u32) -> MeshData {
    let segments = segments.max(3);
    let rings = rings.max(2);
    let half = cylinder_height * 0.5;
    let hemisphere = |centre: f32, first: u32| {
        (first..first + rings + 1).map(move |ring| {
            let polar = ring as f32 / (2 * rings) as f32 * PI;
            let (sin, cos) = polar.sin_cos();
            ProfilePoint::new(radius * sin, centre + radius * cos, sin, cos)
        })
    };
    let profile: Vec<ProfilePoint> = hemisphere(half, 0)
        .chain(hemisphere(-half, rings))
        .collect();
    let mut mesh = MeshData::default();
    push_revolved(&mut mesh, &profile, segments);
    finish(mesh)
}
