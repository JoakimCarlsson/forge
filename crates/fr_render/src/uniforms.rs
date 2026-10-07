//! The byte layouts the 3D shaders read, mirrored from `common.wgsl` and `mesh.wgsl`.

use fr_assets::{AlphaMode, MaterialData};
use fr_core::{DirectionalLight, Light, Mat4, Vec3};

use crate::scene::{AmbientLight, Camera};
use crate::shadows::{CASCADES, LOCAL_LAYERS, LocalShadow, ShadowPlan};

/// How many directional lights the shaders shade with.
pub(crate) const MAX_DIRECTIONAL: usize = 4;

/// How many point and spot lights the shaders shade with.
pub(crate) const MAX_LOCAL: usize = 16;

/// The camera uniform: the combined matrix, the position, the exposure and the view direction.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    /// World space to clip space.
    pub(crate) view_proj: [[f32; 4]; 4],
    /// The world position in xyz and the exposure in w.
    pub(crate) position: [f32; 4],
    /// The unit view direction in xyz.
    pub(crate) forward: [f32; 4],
}

impl CameraUniform {
    /// The uniform for `camera` seen through a viewport of `aspect` width over height.
    pub(crate) fn new(camera: &Camera, aspect: f32) -> Self {
        let view_proj = camera.projection(aspect) * camera.view();
        let forward = (camera.target - camera.position).normalize_or(Vec3::NEG_Z);
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            position: camera.position.extend(camera.exposure).to_array(),
            forward: forward.extend(0.0).to_array(),
        }
    }
}

/// One directional light as the shader reads it.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DirectionalUniform {
    /// The unit vector towards the light in xyz.
    pub(crate) direction: [f32; 4],
    /// The linear colour times the intensity in rgb.
    pub(crate) color: [f32; 4],
}

impl DirectionalUniform {
    /// The uniform for `light`; a zero direction points straight down.
    fn new(light: &DirectionalLight) -> Self {
        let toward = (-light.direction).try_normalize().unwrap_or(Vec3::Y);
        Self {
            direction: toward.extend(0.0).to_array(),
            color: (light.color * light.intensity).extend(0.0).to_array(),
        }
    }
}

/// One point or spot light as the shader reads it.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LocalUniform {
    /// The world position in xyz and the range in w.
    pub(crate) position_range: [f32; 4],
    /// The linear colour times the intensity in rgb; w is 1 for a spot light.
    pub(crate) color: [f32; 4],
    /// The unit axis of a spot light in xyz and its cone scale in w.
    pub(crate) direction: [f32; 4],
    /// The first shadow layer in x, minus one for none, the shadow texel size
    /// at unit distance in y and the cone offset in z.
    pub(crate) shadow: [f32; 4],
}

impl LocalUniform {
    /// The uniform for a point or spot `light` with `shadow`, if it was given a map.
    ///
    /// A directional light is not a local one and gives an unlit entry.
    fn new(light: &Light, shadow: Option<LocalShadow>) -> Self {
        let (layer, texel) = shadow.map_or((-1.0, 0.0), |s| (s.first_layer as f32, s.texel));
        match light {
            Light::Directional(_) => Self::default(),
            Light::Point(light) => Self {
                position_range: light.position.extend(light.range).to_array(),
                color: (light.color * light.intensity).extend(0.0).to_array(),
                direction: [0.0; 4],
                shadow: [layer, texel, 0.0, 0.0],
            },
            Light::Spot(light) => {
                let outer = light.outer_angle.cos();
                let inner = light.inner_angle.min(light.outer_angle).cos();
                let scale = 1.0 / (inner - outer).max(1e-3);
                let axis = light.direction.try_normalize().unwrap_or(Vec3::NEG_Y);
                Self {
                    position_range: light.position.extend(light.range).to_array(),
                    color: (light.color * light.intensity).extend(1.0).to_array(),
                    direction: axis.extend(scale).to_array(),
                    shadow: [layer, texel, -outer * scale, 0.0],
                }
            }
        }
    }
}

/// The light uniform: ambient term, shadow cascades and every light.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LightsUniform {
    /// The linear sky colour times the ambient intensity in rgb.
    pub(crate) sky: [f32; 4],
    /// The linear ground colour times the ambient intensity in rgb.
    pub(crate) ground: [f32; 4],
    /// The directional count, the local count, and one plus the index of the
    /// directional light with shadows or zero.
    pub(crate) counts: [u32; 4],
    /// The far view distance of each cascade.
    pub(crate) cascade_splits: [f32; 4],
    /// The world size of one texel of each cascade.
    pub(crate) cascade_texel: [f32; 4],
    /// World space to the clip space of each cascade.
    pub(crate) cascades: [[[f32; 4]; 4]; CASCADES],
    /// The directional lights.
    pub(crate) directional: [DirectionalUniform; MAX_DIRECTIONAL],
    /// The point and spot lights.
    pub(crate) local: [LocalUniform; MAX_LOCAL],
    /// World space to the clip space of each local shadow layer.
    pub(crate) local_shadows: [[[f32; 4]; 4]; LOCAL_LAYERS],
}

/// The lights of a scene sorted into what the shaders have room for.
#[derive(Default)]
pub(crate) struct LightSet {
    /// The directional lights in use.
    pub(crate) directional: Vec<DirectionalLight>,
    /// The point and spot lights in use.
    pub(crate) local: Vec<Light>,
}

impl LightSet {
    /// Sorts `lights`, dropping those past the capacity of each kind.
    pub(crate) fn new(lights: &[Light]) -> Self {
        let mut set = Self::default();
        for light in lights {
            match light {
                Light::Directional(light) if set.directional.len() < MAX_DIRECTIONAL => {
                    set.directional.push(*light);
                }
                Light::Point(_) | Light::Spot(_) if set.local.len() < MAX_LOCAL => {
                    set.local.push(*light);
                }
                _ => {}
            }
        }
        set
    }
}

impl LightsUniform {
    /// The uniform for `ambient`, the sorted `lights` and the `shadows` planned for them.
    pub(crate) fn new(ambient: &AmbientLight, lights: &LightSet, shadows: &ShadowPlan) -> Self {
        let mut directional = [DirectionalUniform::default(); MAX_DIRECTIONAL];
        for (slot, light) in directional.iter_mut().zip(&lights.directional) {
            *slot = DirectionalUniform::new(light);
        }
        let mut local = [LocalUniform::default(); MAX_LOCAL];
        for (index, (slot, light)) in local.iter_mut().zip(&lights.local).enumerate() {
            *slot = LocalUniform::new(light, shadows.local[index]);
        }
        let mut local_shadows = [[[0.0; 4]; 4]; LOCAL_LAYERS];
        for (slot, matrix) in local_shadows.iter_mut().zip(&shadows.local_matrices) {
            *slot = matrix.to_cols_array_2d();
        }
        let mut cascades = [[[0.0; 4]; 4]; CASCADES];
        let mut cascade_splits = [0.0; 4];
        let mut cascade_texel = [0.0; 4];
        let mut sun = 0;
        if let Some((index, fit)) = &shadows.sun {
            sun = index + 1;
            for (slot, matrix) in cascades.iter_mut().zip(&fit.matrices) {
                *slot = matrix.to_cols_array_2d();
            }
            cascade_splits = fit.splits;
            cascade_texel = fit.texels;
        }
        Self {
            sky: (ambient.sky_color * ambient.intensity)
                .extend(0.0)
                .to_array(),
            ground: (ambient.ground_color * ambient.intensity)
                .extend(0.0)
                .to_array(),
            counts: [
                lights.directional.len() as u32,
                lights.local.len() as u32,
                sun as u32,
                0,
            ],
            cascade_splits,
            cascade_texel,
            cascades,
            directional,
            local,
            local_shadows,
        }
    }
}

/// The per-object entry of the storage buffer the vertex shader indexes.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ObjectData {
    /// Object space to world space.
    pub(crate) model: [[f32; 4]; 4],
    /// The inverse transpose of the model matrix, for normals.
    pub(crate) normal_matrix: [[f32; 4]; 4],
}

impl ObjectData {
    /// The entry for an object placed by `model`.
    pub(crate) fn new(model: Mat4) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            normal_matrix: model.inverse().transpose().to_cols_array_2d(),
        }
    }
}

/// The material uniform of group 1.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MaterialUniform {
    /// The linear base colour factor.
    pub(crate) base_color: [f32; 4],
    /// The linear emissive factor in rgb.
    pub(crate) emissive: [f32; 4],
    /// The metalness factor.
    pub(crate) metallic: f32,
    /// The roughness factor.
    pub(crate) roughness: f32,
    /// The normal texture strength.
    pub(crate) normal_scale: f32,
    /// The occlusion texture strength.
    pub(crate) occlusion_strength: f32,
    /// The alpha below which a masked fragment is discarded.
    pub(crate) alpha_cutoff: f32,
    /// 0 for opaque, 1 for mask, 2 for blend.
    pub(crate) alpha_mode: u32,
    /// Padding to a 16-byte multiple.
    pub(crate) padding: [u32; 2],
}

impl MaterialUniform {
    /// The uniform for the factors of `material`.
    pub(crate) fn new<T>(material: &MaterialData<T>) -> Self {
        let (alpha_mode, alpha_cutoff) = match material.alpha_mode {
            AlphaMode::Opaque => (0, 0.0),
            AlphaMode::Mask(cutoff) => (1, cutoff),
            AlphaMode::Blend => (2, 0.0),
        };
        Self {
            base_color: material.base_color.to_array(),
            emissive: material.emissive.extend(0.0).to_array(),
            metallic: material.metallic,
            roughness: material.roughness,
            normal_scale: material.normal_scale,
            occlusion_strength: material.occlusion_strength,
            alpha_cutoff,
            alpha_mode,
            padding: [0; 2],
        }
    }
}

/// One mesh vertex as the mesh pipeline reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    /// The object-space position.
    pub(crate) position: [f32; 3],
    /// The object-space normal.
    pub(crate) normal: [f32; 3],
    /// The object-space tangent with the bitangent sign in w.
    pub(crate) tangent: [f32; 4],
    /// The first texture coordinate set.
    pub(crate) uv: [f32; 2],
}

impl Vertex {
    /// The vertex attributes the mesh shader expects, in declaration order.
    pub(crate) const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x4,
        3 => Float32x2,
    ];
}
