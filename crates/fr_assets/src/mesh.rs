//! Indexed triangle geometry with the attributes the renderer reads.

use fr_core::{Vec2, Vec3, Vec4};

/// An indexed triangle list with a normal, tangent and texture coordinate per vertex.
///
/// Counter-clockwise triangles face the viewer; texture coordinates have their
/// origin at the top-left of the image. The attribute vectors all hold one
/// entry per vertex.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    /// The vertex positions.
    pub positions: Vec<Vec3>,
    /// The unit vertex normals.
    pub normals: Vec<Vec3>,
    /// The unit tangents in `xyz` and the bitangent sign in `w`.
    pub tangents: Vec<Vec4>,
    /// The first texture coordinate set.
    pub uvs: Vec<Vec2>,
    /// Three vertex indices per triangle.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Whether every attribute has one entry per vertex and every index is in range.
    pub fn is_consistent(&self) -> bool {
        let count = self.positions.len();
        self.normals.len() == count
            && self.tangents.len() == count
            && self.uvs.len() == count
            && self.indices.len().is_multiple_of(3)
            && self.indices.iter().all(|&index| (index as usize) < count)
    }

    /// Replaces the normals with smooth, area-weighted ones from the triangles.
    pub fn compute_normals(&mut self) {
        let mut normals = vec![Vec3::ZERO; self.positions.len()];
        for triangle in self.indices.as_chunks::<3>().0 {
            let [a, b, c] = corner_positions(&self.positions, triangle);
            let weighted = (b - a).cross(c - a);
            for &index in triangle {
                normals[index as usize] += weighted;
            }
        }
        self.normals = normals
            .into_iter()
            .map(|normal| normal.try_normalize().unwrap_or(Vec3::Y))
            .collect();
    }

    /// Replaces the tangents with ones derived from the texture coordinates.
    ///
    /// The bitangent sign follows the glTF convention, where the normal
    /// texture's green channel points up the image. Vertices without usable
    /// texture coordinates get any tangent perpendicular to their normal.
    pub fn compute_tangents(&mut self) {
        let count = self.positions.len();
        let mut tangents = vec![Vec3::ZERO; count];
        let mut bitangents = vec![Vec3::ZERO; count];
        for triangle in self.indices.as_chunks::<3>().0 {
            let [a, b, c] = corner_positions(&self.positions, triangle);
            let edge_1 = b - a;
            let edge_2 = c - a;
            let uv_1 = self.uvs[triangle[1] as usize] - self.uvs[triangle[0] as usize];
            let uv_2 = self.uvs[triangle[2] as usize] - self.uvs[triangle[0] as usize];
            let determinant = uv_1.x * uv_2.y - uv_2.x * uv_1.y;
            if determinant.abs() <= f32::EPSILON {
                continue;
            }
            let tangent = (edge_1 * uv_2.y - edge_2 * uv_1.y) / determinant;
            let bitangent = (edge_2 * uv_1.x - edge_1 * uv_2.x) / determinant;
            for &index in triangle {
                tangents[index as usize] += tangent;
                bitangents[index as usize] += bitangent;
            }
        }
        self.tangents = (0..count)
            .map(|index| {
                orthonormal_tangent(self.normals[index], tangents[index], bitangents[index])
            })
            .collect();
    }

    /// The smallest and largest corner of the box around every vertex, or
    /// zeros for a mesh without vertices.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let Some(&first) = self.positions.first() else {
            return (Vec3::ZERO, Vec3::ZERO);
        };
        self.positions
            .iter()
            .fold((first, first), |(low, high), &position| {
                (low.min(position), high.max(position))
            })
    }
}

/// The three positions a triangle's indices select.
fn corner_positions(positions: &[Vec3], triangle: &[u32]) -> [Vec3; 3] {
    [
        positions[triangle[0] as usize],
        positions[triangle[1] as usize],
        positions[triangle[2] as usize],
    ]
}

/// A unit tangent for `normal` along `tangent`, signed by `bitangent`.
///
/// Falls back to any vector perpendicular to the normal when `tangent` is
/// degenerate.
fn orthonormal_tangent(normal: Vec3, tangent: Vec3, bitangent: Vec3) -> Vec4 {
    let projected = tangent - normal * normal.dot(tangent);
    let unit = projected
        .try_normalize()
        .unwrap_or_else(|| normal.any_orthonormal_vector());
    let sign = if normal.cross(unit).dot(bitangent) > 0.0 {
        -1.0
    } else {
        1.0
    };
    unit.extend(sign)
}
