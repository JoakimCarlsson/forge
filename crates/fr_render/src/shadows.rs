//! Shadow maps: cascades for the sun, a layer per spot light and six per
//! point light, and the depth-only passes that fill them.
//!
//! Planning ([`plan_shadows`]) is pure maths from the camera and the lights to
//! matrices; [`ShadowMaps`] owns the textures and records the passes.

use std::ops::Range;

use fr_camera::Camera;
use fr_light::{DirectionalLight, Light};
use fr_math::{Mat4, Vec3, look_at, orthographic, perspective};
use fr_mesh::MeshId;

use crate::resources::Resources;
use crate::uniforms::{LightSet, Vertex};

/// The number of cascades the sun's shadow is split into.
pub(crate) const CASCADES: usize = 3;

/// The width and height of each cascade, in texels.
const CASCADE_SIZE: u32 = 2048;

/// The number of layers shared by every spot light and cube face of a point light.
pub(crate) const LOCAL_LAYERS: usize = 12;

/// The width and height of each local layer, in texels.
const LOCAL_SIZE: u32 = 1024;

/// The format of every shadow map.
const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// How far behind a cascade, towards the sun, objects still cast shadows into it.
const CASTER_MARGIN: f32 = 100.0;

/// How far the cascade splits lean from even towards logarithmic spacing.
const SPLIT_LAMBDA: f32 = 0.8;

/// The field of view of one face of a point light's shadow cube; a little over
/// ninety degrees so that filtering at a seam finds data.
const POINT_FACE_FOV: f32 = 92.0 * std::f32::consts::PI / 180.0;

/// The nearest distance a local light's shadow reaches.
const LOCAL_NEAR: f32 = 0.05;

/// How far a local light with no range casts shadows.
const LOCAL_UNLIMITED_FAR: f32 = 100.0;

/// The shadow-map faces of a point light: the direction each looks along and its up vector.
const CUBE_FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::Y),
    (Vec3::NEG_X, Vec3::Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
    (Vec3::Z, Vec3::Y),
    (Vec3::NEG_Z, Vec3::Y),
];

/// The matrices, split distances and texel sizes of the sun's cascades.
pub(crate) struct CascadeFit {
    /// World space to the clip space of each cascade.
    pub(crate) matrices: [Mat4; CASCADES],
    /// The far view distance of each cascade; the fourth entry is unused.
    pub(crate) splits: [f32; 4],
    /// The world size of one texel of each cascade; the fourth entry is unused.
    pub(crate) texels: [f32; 4],
}

/// Where a local light's shadow lives in the layered local map.
#[derive(Clone, Copy)]
pub(crate) struct LocalShadow {
    /// The first of the one or six layers it owns.
    pub(crate) first_layer: usize,
    /// The world size of one texel at one unit of distance from the light.
    pub(crate) texel: f32,
}

/// Every shadow matrix of a frame, ready for the uniform and the passes.
#[derive(Default)]
pub(crate) struct ShadowPlan {
    /// The index of the shadow-casting directional light and its cascades.
    pub(crate) sun: Option<(usize, CascadeFit)>,
    /// The shadow of each local light, in the order of the light set.
    pub(crate) local: Vec<Option<LocalShadow>>,
    /// The matrix of each used local layer.
    pub(crate) local_matrices: Vec<Mat4>,
}

/// Plans the cascades and local shadows for `lights` seen by `camera` at `aspect`.
pub(crate) fn plan_shadows(camera: &Camera, aspect: f32, lights: &LightSet) -> ShadowPlan {
    let sun = lights
        .directional
        .iter()
        .position(|light| light.cast_shadows)
        .map(|index| {
            (
                index,
                fit_cascades(camera, aspect, &lights.directional[index]),
            )
        });
    let mut plan = ShadowPlan {
        sun,
        ..ShadowPlan::default()
    };
    for light in &lights.local {
        let shadow = plan_local(light, &mut plan.local_matrices);
        plan.local.push(shadow);
    }
    plan
}

/// Fits `CASCADES` orthographic maps to slices of the camera's frustum.
fn fit_cascades(camera: &Camera, aspect: f32, light: &DirectionalLight) -> CascadeFit {
    let near = camera.projection.near().max(1e-3);
    let far = light
        .shadow_distance
        .clamp(near * 2.0, camera.projection.far().max(near * 2.0));
    let view = camera.view();
    let direction = light.direction.try_normalize().unwrap_or(Vec3::NEG_Y);
    let up = if direction.dot(Vec3::Y).abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let light_view = look_at(Vec3::ZERO, direction, up);

    let mut fit = CascadeFit {
        matrices: [Mat4::IDENTITY; CASCADES],
        splits: [0.0; 4],
        texels: [0.0; 4],
    };
    let mut slice_near = near;
    for index in 0..CASCADES {
        let fraction = (index + 1) as f32 / CASCADES as f32;
        let logarithmic = near * (far / near).powf(fraction);
        let uniform = near + (far - near) * fraction;
        let slice_far = SPLIT_LAMBDA * logarithmic + (1.0 - SPLIT_LAMBDA) * uniform;
        let corners = slice_corners(camera, aspect, &view, slice_near, slice_far);
        let (matrix, texel) = fit_slice(&corners, &light_view);
        fit.matrices[index] = matrix;
        fit.texels[index] = texel;
        fit.splits[index] = slice_far;
        slice_near = slice_far;
    }
    fit
}

/// The world-space corners of the camera's frustum between `near` and `far`.
fn slice_corners(camera: &Camera, aspect: f32, view: &Mat4, near: f32, far: f32) -> [Vec3; 8] {
    let inverse = (camera.projection.with_clip(near, far).matrix(aspect) * *view).inverse();
    std::array::from_fn(|index| {
        let ndc = Vec3::new(
            if index & 1 == 0 { -1.0 } else { 1.0 },
            if index & 2 == 0 { -1.0 } else { 1.0 },
            if index & 4 == 0 { 0.0 } else { 1.0 },
        );
        inverse.project_point3(ndc)
    })
}

/// The orthographic matrix enclosing `corners` as seen through `light_view`, and its texel size.
///
/// The enclosing box is the corners' bounding sphere, whose size does not
/// change as the camera turns, with its centre snapped to whole texels so
/// edges do not shimmer as the camera moves.
fn fit_slice(corners: &[Vec3; 8], light_view: &Mat4) -> (Mat4, f32) {
    let center = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
    let radius = corners
        .iter()
        .map(|corner| corner.distance(center))
        .fold(0.0, f32::max);
    let radius = (radius * 16.0).ceil() / 16.0;
    let texel = 2.0 * radius / CASCADE_SIZE as f32;
    let mut middle = light_view.transform_point3(center);
    middle.x = (middle.x / texel).round() * texel;
    middle.y = (middle.y / texel).round() * texel;
    let projection = orthographic(
        middle.x - radius,
        middle.x + radius,
        middle.y - radius,
        middle.y + radius,
        -(middle.z + radius + CASTER_MARGIN),
        -(middle.z - radius),
    );
    (projection * *light_view, texel)
}

/// Gives `light` shadow layers, appending their matrices, if it casts shadows and layers remain.
fn plan_local(light: &Light, matrices: &mut Vec<Mat4>) -> Option<LocalShadow> {
    let first_layer = matrices.len();
    match light {
        Light::Directional(_) => None,
        Light::Spot(spot) if spot.cast_shadows && first_layer < LOCAL_LAYERS => {
            let direction = spot.direction.try_normalize()?;
            let fov = (2.0 * spot.outer_angle).clamp(0.02, 3.0);
            let up = if direction.dot(Vec3::Y).abs() > 0.99 {
                Vec3::Z
            } else {
                Vec3::Y
            };
            matrices.push(face_matrix(spot.position, direction, up, fov, spot.range));
            Some(LocalShadow {
                first_layer,
                texel: texel_at_unit(fov),
            })
        }
        Light::Point(point) if point.cast_shadows && first_layer + 6 <= LOCAL_LAYERS => {
            for (direction, up) in CUBE_FACES {
                matrices.push(face_matrix(
                    point.position,
                    direction,
                    up,
                    POINT_FACE_FOV,
                    point.range,
                ));
            }
            Some(LocalShadow {
                first_layer,
                texel: texel_at_unit(POINT_FACE_FOV),
            })
        }
        _ => None,
    }
}

/// The world size of one local texel at one unit from a light whose map spans `fov` radians.
fn texel_at_unit(fov: f32) -> f32 {
    2.0 * (fov * 0.5).tan() / LOCAL_SIZE as f32
}

/// World space to the clip space of a square view from `position` along `direction`.
fn face_matrix(position: Vec3, direction: Vec3, up: Vec3, fov: f32, range: f32) -> Mat4 {
    let far = if range > 0.0 {
        range
    } else {
        LOCAL_UNLIMITED_FAR
    };
    perspective(fov, 1.0, LOCAL_NEAR, far) * look_at(position, position + direction, up)
}

/// Which texture and layer a shadow pass renders into.
#[derive(Clone, Copy)]
enum Layer {
    /// A cascade of the sun.
    Cascade(usize),
    /// A layer of the local map.
    Local(usize),
}

/// The shadow textures, the pipeline that fills them and the passes of the frame.
pub(crate) struct ShadowMaps {
    /// The cascade array as sampled by the mesh shader.
    cascade_array: wgpu::TextureView,
    /// The local layer array as sampled by the mesh shader.
    local_array: wgpu::TextureView,
    /// The comparison sampler the mesh shader filters with.
    sampler: wgpu::Sampler,
    /// One render target view per cascade.
    cascade_layers: Vec<wgpu::TextureView>,
    /// One render target view per local layer.
    local_layers: Vec<wgpu::TextureView>,
    /// The depth-only pipeline.
    pipeline: wgpu::RenderPipeline,
    /// The layout of the pass bindings: a light matrix and the objects.
    layout: wgpu::BindGroupLayout,
    /// The pass bindings, rebuilt whenever the objects buffer is.
    group: wgpu::BindGroup,
    /// The light matrices of the frame's passes, one per aligned slot.
    matrices: wgpu::Buffer,
    /// The distance in bytes between matrix slots.
    stride: usize,
    /// The matrix bytes of the frame, reused between frames.
    staging: Vec<u8>,
    /// The passes of the frame in order, with the matrix slot of each.
    passes: Vec<(Layer, usize)>,
}

/// Source of the shadow depth shader: the shared and depth parts.
const SHADOW_SHADER: &str = concat!(
    include_str!("shaders/common.wgsl"),
    "\n",
    include_str!("shaders/shadow_depth.wgsl"),
);

impl ShadowMaps {
    /// Creates the textures and pipeline, binding `objects` as the per-object storage.
    pub(crate) fn new(device: &wgpu::Device, objects: &wgpu::Buffer) -> Self {
        let (cascade_texture, cascade_layers) =
            layered_map(device, "sun shadow", CASCADE_SIZE, CASCADES as u32);
        let (local_texture, local_layers) =
            layered_map(device, "local shadow", LOCAL_SIZE, LOCAL_LAYERS as u32);
        let array_view = |texture: &wgpu::Texture| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            })
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let stride = (device.limits().min_uniform_buffer_offset_alignment as usize)
            .max(size_of::<[[f32; 4]; 4]>());
        let matrices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow matrices"),
            size: (stride * (CASCADES + LOCAL_LAYERS)) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = pass_layout(device);
        let group = pass_group(device, &layout, &matrices, objects);
        let pipeline = build_pipeline(device, &layout);
        Self {
            cascade_array: array_view(&cascade_texture),
            local_array: array_view(&local_texture),
            sampler,
            cascade_layers,
            local_layers,
            pipeline,
            layout,
            group,
            matrices,
            stride,
            staging: Vec::new(),
            passes: Vec::new(),
        }
    }

    /// The cascade array view, to bind for sampling.
    pub(crate) fn cascade_array(&self) -> &wgpu::TextureView {
        &self.cascade_array
    }

    /// The local layer array view, to bind for sampling.
    pub(crate) fn local_array(&self) -> &wgpu::TextureView {
        &self.local_array
    }

    /// The comparison sampler, to bind for sampling.
    pub(crate) fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Rebinds the passes to a new `objects` buffer.
    pub(crate) fn rebind_objects(&mut self, device: &wgpu::Device, objects: &wgpu::Buffer) {
        self.group = pass_group(device, &self.layout, &self.matrices, objects);
    }

    /// Uploads the matrices of `plan` and lists the passes that will use them.
    pub(crate) fn upload(&mut self, queue: &wgpu::Queue, plan: &ShadowPlan) {
        self.passes.clear();
        self.staging.clear();
        let mut push = |passes: &mut Vec<(Layer, usize)>, layer, matrix: &Mat4| {
            passes.push((layer, self.staging.len() / self.stride));
            self.staging
                .extend_from_slice(bytemuck::bytes_of(&matrix.to_cols_array_2d()));
            self.staging.resize(passes.len() * self.stride, 0);
        };
        if let Some((_, fit)) = &plan.sun {
            for (index, matrix) in fit.matrices.iter().enumerate() {
                push(&mut self.passes, Layer::Cascade(index), matrix);
            }
        }
        for (index, matrix) in plan.local_matrices.iter().enumerate() {
            push(&mut self.passes, Layer::Local(index), matrix);
        }
        if !self.staging.is_empty() {
            queue.write_buffer(&self.matrices, 0, &self.staging);
        }
    }

    /// Records one depth pass per planned layer, drawing every shadow-casting
    /// batch of `draws` (a mesh and the range of objects drawn with it).
    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        resources: &Resources,
        draws: &[(MeshId, Range<u32>)],
    ) {
        for &(layer, slot) in &self.passes {
            let view = match layer {
                Layer::Cascade(index) => &self.cascade_layers[index],
                Layer::Local(index) => &self.local_layers[index],
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("forge shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.group, &[(slot * self.stride) as u32]);
            for (mesh, objects) in draws {
                let Some(mesh) = resources.mesh(*mesh) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, objects.clone());
            }
        }
    }
}

/// Creates a depth texture of `layers` square layers of `size`, with a view of each layer.
fn layered_map(
    device: &wgpu::Device,
    label: &str,
    size: u32,
    layers: u32,
) -> (wgpu::Texture, Vec<wgpu::TextureView>) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SHADOW_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let views = (0..layers)
        .map(|layer| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        })
        .collect();
    (texture, views)
}

/// The layout of the pass bindings: a light matrix at a dynamic offset, then the objects.
fn pass_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shadow pass"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(size_of::<[[f32; 4]; 4]>() as u64),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

/// Binds one matrix slot of `matrices` and all of `objects` to `layout`.
fn pass_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    matrices: &wgpu::Buffer,
    objects: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow pass"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: matrices,
                    offset: 0,
                    size: wgpu::BufferSize::new(size_of::<[[f32; 4]; 4]>() as u64),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: objects.as_entire_binding(),
            },
        ],
    })
}

/// Builds the depth-only pipeline with a slope-scaled bias.
fn build_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shadow depth"),
        source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow depth"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shadow depth"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3],
            })],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: SHADOW_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: None,
        multiview_mask: None,
        cache: None,
    })
}
