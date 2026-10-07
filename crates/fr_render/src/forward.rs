//! The forward 3D pass: pipelines, per-frame bindings and draw batching.
//!
//! One pipeline exists per combination of blending and face culling, never per
//! object. Every object's transform lives in one storage buffer indexed by the
//! instance, so objects sharing a mesh and material collapse into one draw.
//! The shadow maps are filled first, from the same batches, in the same encoder.

use std::ops::Range;

use fr_material::MaterialId;
use fr_mesh::MeshId;

use crate::resources::Resources;
use crate::scene::Scene;
use crate::shadows::{ShadowMaps, plan_shadows};
use crate::targets::{DEPTH_FORMAT, PixelRect, SceneTargets};
use crate::uniforms::{CameraUniform, LightSet, LightsUniform, ObjectData, Vertex};

/// The mesh shader: the common, output, PBR, shadow, light and mesh parts
/// joined in that order, with a placeholder for the output encoding.
const MESH_SHADER: &str = concat!(
    include_str!("shaders/common.wgsl"),
    "\n",
    include_str!("shaders/output.wgsl"),
    "\n",
    include_str!("shaders/pbr.wgsl"),
    "\n",
    include_str!("shaders/shadows.wgsl"),
    "\n",
    include_str!("shaders/lights.wgsl"),
    "\n",
    include_str!("shaders/mesh.wgsl"),
);

/// The number of pipeline variants: blended or not, culled or not.
const VARIANTS: usize = 4;

/// One draw call: a run of instances sharing a pipeline, material and mesh.
struct Batch {
    /// The index of the pipeline variant.
    variant: usize,
    /// The material bound.
    material: MaterialId,
    /// The mesh drawn.
    mesh: MeshId,
    /// The entries of the object buffer drawn.
    objects: Range<u32>,
}

/// One scene instance resolved to what sorting and batching need.
struct Draw {
    /// The index of the pipeline variant.
    variant: usize,
    /// Whether the draw belongs to the sorted blended group.
    blend: bool,
    /// The material bound.
    material: MaterialId,
    /// The mesh drawn.
    mesh: MeshId,
    /// The distance along the view direction, used to sort blended draws.
    depth: f32,
    /// The object's matrices.
    object: ObjectData,
}

/// Renders a [`Scene`] into a multisampled, depth-tested target.
pub(crate) struct ForwardPass {
    /// The number of samples per pixel.
    samples: u32,
    /// The depth and colour targets, recreated when the size changes.
    targets: SceneTargets,
    /// The colour format rendered to.
    format: wgpu::TextureFormat,
    /// The part of the target the prepared scene is drawn into.
    viewport: PixelRect,
    /// The camera uniform.
    camera: wgpu::Buffer,
    /// The lights uniform.
    lights: wgpu::Buffer,
    /// The storage buffer of per-object matrices.
    objects: wgpu::Buffer,
    /// The capacity of `objects` in entries.
    capacity: usize,
    /// The layout of group 0.
    frame_layout: wgpu::BindGroupLayout,
    /// Group 0: camera, lights and objects.
    frame_group: wgpu::BindGroup,
    /// The pipeline variants, indexed by [`variant_index`].
    pipelines: Vec<wgpu::RenderPipeline>,
    /// The object matrices of the frame, in draw order, reused between frames.
    object_data: Vec<ObjectData>,
    /// The draw calls of the frame, reused between frames.
    batches: Vec<Batch>,
    /// The sorted draws of the frame, reused between frames.
    draws: Vec<Draw>,
    /// The shadow maps and the passes that fill them.
    shadows: ShadowMaps,
    /// The mesh and object range of each batch that casts shadows, reused between frames.
    shadow_draws: Vec<(MeshId, Range<u32>)>,
}

/// Whether pipeline variant `variant` blends, and so neither writes depth nor casts shadows.
fn is_blended(variant: usize) -> bool {
    variant >= 2
}

/// The index of the pipeline variant for a blend and double-sided combination.
fn variant_index(blend: bool, double_sided: bool) -> usize {
    usize::from(blend) * 2 + usize::from(double_sided)
}

impl ForwardPass {
    /// Creates the pass for a target of `format` at `size`, multisampled `samples` times.
    pub(crate) fn new(
        device: &wgpu::Device,
        resources: &Resources,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        samples: u32,
    ) -> Self {
        let uniform = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let camera = uniform("camera", size_of::<CameraUniform>() as u64);
        let lights = uniform("lights", size_of::<LightsUniform>() as u64);
        let objects = create_objects(device, 1);
        let frame_layout = frame_layout(device);
        let shadows = ShadowMaps::new(device, &objects);
        let frame_group = frame_group(device, &frame_layout, &camera, &lights, &objects, &shadows);
        let pipelines = build_pipelines(device, resources, &frame_layout, format, samples);
        Self {
            samples,
            targets: SceneTargets::new(device, format, size, samples),
            format,
            viewport: PixelRect::full(size),
            camera,
            lights,
            objects,
            capacity: 1,
            frame_layout,
            frame_group,
            pipelines,
            object_data: Vec::new(),
            batches: Vec::new(),
            draws: Vec::new(),
            shadows,
            shadow_draws: Vec::new(),
        }
    }

    /// Uploads `scene`'s camera, lights and objects and plans the draw calls
    /// for a target of `size` physical pixels, drawn into `viewport` of it.
    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &Resources,
        scene: &Scene,
        size: (u32, u32),
        viewport: PixelRect,
    ) {
        if !self.targets.matches(size) {
            self.targets = SceneTargets::new(device, self.format, size, self.samples);
        }
        self.viewport = viewport;
        let aspect = viewport.aspect();
        queue.write_buffer(
            &self.camera,
            0,
            bytemuck::bytes_of(&CameraUniform::new(&scene.camera, aspect)),
        );
        let lights = LightSet::new(&scene.lights);
        let plan = plan_shadows(&scene.camera, aspect, &lights);
        queue.write_buffer(
            &self.lights,
            0,
            bytemuck::bytes_of(&LightsUniform::new(&scene.ambient, &lights, &plan)),
        );
        self.shadows.upload(queue, &plan);

        self.collect_draws(resources, scene);
        self.plan_batches();
        if self.object_data.len() > self.capacity {
            self.capacity = self.object_data.len().next_power_of_two();
            self.objects = create_objects(device, self.capacity);
            self.frame_group = frame_group(
                device,
                &self.frame_layout,
                &self.camera,
                &self.lights,
                &self.objects,
                &self.shadows,
            );
            self.shadows.rebind_objects(device, &self.objects);
        }
        if !self.object_data.is_empty() {
            queue.write_buffer(&self.objects, 0, bytemuck::cast_slice(&self.object_data));
        }
    }

    /// Resolves the scene's instances to draws, opaque ones first by material
    /// and mesh, then blended ones from far to near.
    fn collect_draws(&mut self, resources: &Resources, scene: &Scene) {
        self.draws.clear();
        let forward = (scene.camera.target - scene.camera.position).normalize_or_zero();
        for instance in &scene.instances {
            let (Some(material), Some(_)) = (
                resources.material(instance.material),
                resources.mesh(instance.mesh),
            ) else {
                continue;
            };
            let matrix = instance.transform.matrix();
            self.draws.push(Draw {
                variant: variant_index(material.blend, material.double_sided),
                blend: material.blend,
                material: instance.material,
                mesh: instance.mesh,
                depth: (instance.transform.translation - scene.camera.position).dot(forward),
                object: ObjectData::new(matrix),
            });
        }
        self.draws.sort_by(|a, b| {
            a.blend.cmp(&b.blend).then_with(|| {
                if a.blend {
                    b.depth.total_cmp(&a.depth)
                } else {
                    (a.variant, a.material, a.mesh).cmp(&(b.variant, b.material, b.mesh))
                }
            })
        });
    }

    /// Turns the sorted draws into object entries and batches of equal neighbours,
    /// and lists the batches that cast shadows.
    fn plan_batches(&mut self) {
        self.object_data.clear();
        self.batches.clear();
        for draw in &self.draws {
            let index = self.object_data.len() as u32;
            self.object_data.push(draw.object);
            match self.batches.last_mut() {
                Some(last)
                    if last.variant == draw.variant
                        && last.material == draw.material
                        && last.mesh == draw.mesh =>
                {
                    last.objects.end = index + 1;
                }
                _ => self.batches.push(Batch {
                    variant: draw.variant,
                    material: draw.material,
                    mesh: draw.mesh,
                    objects: index..index + 1,
                }),
            }
        }
        self.shadow_draws.clear();
        self.shadow_draws.extend(
            self.batches
                .iter()
                .filter(|batch| !is_blended(batch.variant))
                .map(|batch| (batch.mesh, batch.objects.clone())),
        );
    }

    /// Records the pass drawing the prepared scene into `target`, clearing it to `clear`.
    ///
    /// With multisampling the scene is resolved into `target`; either way the
    /// pass ends with `target` holding the finished 3D image, ready for the UI.
    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clear: wgpu::Color,
        resources: &Resources,
    ) {
        self.shadows.encode(encoder, resources, &self.shadow_draws);
        let (view, resolve_target, store) = match self.targets.color() {
            Some(color) => (color, Some(target), wgpu::StoreOp::Discard),
            None => (target, None, wgpu::StoreOp::Store),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("forge scene pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: self.targets.depth(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_viewport(
            self.viewport.x as f32,
            self.viewport.y as f32,
            self.viewport.width as f32,
            self.viewport.height as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(
            self.viewport.x,
            self.viewport.y,
            self.viewport.width,
            self.viewport.height,
        );
        pass.set_bind_group(0, &self.frame_group, &[]);
        let mut bound_variant = None;
        let mut bound_material = None;
        for batch in &self.batches {
            let (Some(mesh), Some(material)) = (
                resources.mesh(batch.mesh),
                resources.material(batch.material),
            ) else {
                continue;
            };
            if bound_variant != Some(batch.variant) {
                pass.set_pipeline(&self.pipelines[batch.variant]);
                bound_variant = Some(batch.variant);
            }
            if bound_material != Some(batch.material) {
                pass.set_bind_group(1, &material.group, &[]);
                bound_material = Some(batch.material);
            }
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, batch.objects.clone());
        }
    }
}

/// Creates the object storage buffer with room for `capacity` entries.
fn create_objects(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("objects"),
        size: (capacity * size_of::<ObjectData>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// The layout of group 0: camera and lights uniforms, the object storage, then
/// the two shadow arrays and their comparison sampler.
fn frame_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let uniform = wgpu::BufferBindingType::Uniform;
    let stages = wgpu::ShaderStages::VERTEX_FRAGMENT;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scene frame"),
        entries: &[
            entry(0, stages, uniform),
            entry(1, wgpu::ShaderStages::FRAGMENT, uniform),
            entry(
                2,
                wgpu::ShaderStages::VERTEX,
                wgpu::BufferBindingType::Storage { read_only: true },
            ),
            shadow_texture_entry(3),
            shadow_texture_entry(4),
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ],
    })
}

/// The layout entry of a layered depth texture sampled by the fragment stage at `binding`.
fn shadow_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    }
}

/// Binds the camera, lights, object buffers and shadow maps as group 0.
fn frame_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    camera: &wgpu::Buffer,
    lights: &wgpu::Buffer,
    objects: &wgpu::Buffer,
    shadows: &ShadowMaps,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scene frame"),
        layout,
        entries: &[
            buffer_entry(0, camera),
            buffer_entry(1, lights),
            buffer_entry(2, objects),
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(shadows.cascade_array()),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(shadows.local_array()),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(shadows.sampler()),
            },
        ],
    })
}

/// The bind group entry binding all of `buffer` at `binding`.
fn buffer_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

/// Builds every pipeline variant of the mesh shader.
fn build_pipelines(
    device: &wgpu::Device,
    resources: &Resources,
    frame_layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    samples: u32,
) -> Vec<wgpu::RenderPipeline> {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mesh"),
        source: wgpu::ShaderSource::Wgsl(
            MESH_SHADER
                .replace(
                    "ENCODE_SRGB",
                    if format.is_srgb() { "false" } else { "true" },
                )
                .into(),
        ),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mesh"),
        bind_group_layouts: &[Some(frame_layout), Some(resources.material_layout())],
        immediate_size: 0,
    });
    (0..VARIANTS)
        .map(|variant| {
            let blend = is_blended(variant);
            let double_sided = variant % 2 == 1;
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("mesh"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vertex"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &Vertex::ATTRIBUTES,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: (!double_sided).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!blend),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fragment"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: blend.then_some(ALPHA_BLEND),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        })
        .collect()
}

/// Straight-alpha blending that leaves the target's own alpha untouched.
const ALPHA_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};
