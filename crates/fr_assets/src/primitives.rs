//! Built-in shapes: a cube, a sphere and a plane.

use std::f32::consts::{PI, TAU};

use fr_core::{Vec2, Vec3};

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
