//! The GPU-side tables behind [`MeshId`], [`MaterialId`] and [`TextureId`].

use fr_image::{Filter, ImageData, SamplerData, TextureId, Wrap};
use fr_material::{AlphaMode, MaterialData, MaterialId};
use fr_mesh::{MeshData, MeshId};
use wgpu::util::DeviceExt;

use crate::error::RenderError;
use crate::mipmaps::{MipmapGenerator, mip_level_count};
use crate::uniforms::{MaterialUniform, Vertex};

/// The anisotropy requested of textures filtered linearly everywhere.
const ANISOTROPY: u16 = 16;

/// A mesh's vertex and index buffers.
pub(crate) struct GpuMesh {
    /// The vertices.
    pub(crate) vertices: wgpu::Buffer,
    /// The 32-bit triangle indices.
    pub(crate) indices: wgpu::Buffer,
    /// How many indices are drawn.
    pub(crate) index_count: u32,
}

/// A texture view with the sampler it is read with.
pub(crate) struct GpuTexture {
    /// The view over every mip level.
    view: wgpu::TextureView,
    /// The sampler for the view.
    sampler: wgpu::Sampler,
}

/// A material's bind group and the pipeline state it selects.
pub(crate) struct GpuMaterial {
    /// Group 1: the uniform, five textures and five samplers.
    pub(crate) group: wgpu::BindGroup,
    /// Whether the material is drawn in the sorted blended pass.
    pub(crate) blend: bool,
    /// Whether back faces are drawn.
    pub(crate) double_sided: bool,
}

/// Every mesh, material and texture the renderer has been given.
pub(crate) struct Resources {
    /// The layout of a material's bind group.
    material_layout: wgpu::BindGroupLayout,
    /// Builds mip chains for new textures.
    mipmaps: MipmapGenerator,
    /// A white texel, read where a material has no texture.
    white: GpuTexture,
    /// A flat tangent-space normal, read where a material has no normal texture.
    flat_normal: GpuTexture,
    /// The meshes, indexed by [`MeshId`].
    meshes: Vec<Option<GpuMesh>>,
    /// The materials, indexed by [`MaterialId`].
    materials: Vec<Option<GpuMaterial>>,
    /// The textures, indexed by [`TextureId`].
    textures: Vec<Option<GpuTexture>>,
}

impl Resources {
    /// Creates empty tables and the fallback textures on `device`.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mipmaps = MipmapGenerator::new(device);
        let fallback = |label, rgba| {
            upload_texture(
                device,
                queue,
                &mipmaps,
                &ImageData::solid(rgba),
                SamplerData::default(),
                false,
                label,
            )
        };
        Self {
            material_layout: material_layout(device),
            white: fallback("white", [255; 4]),
            flat_normal: fallback("flat normal", [128, 128, 255, 255]),
            mipmaps,
            meshes: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
        }
    }

    /// The layout of a material's bind group.
    pub(crate) fn material_layout(&self) -> &wgpu::BindGroupLayout {
        &self.material_layout
    }

    /// The mesh behind `id`, if it exists.
    pub(crate) fn mesh(&self, id: MeshId) -> Option<&GpuMesh> {
        self.meshes.get(id.index()).and_then(Option::as_ref)
    }

    /// The material behind `id`, if it exists.
    pub(crate) fn material(&self, id: MaterialId) -> Option<&GpuMaterial> {
        self.materials.get(id.index()).and_then(Option::as_ref)
    }

    /// Releases the GPU buffers of a mesh without reusing its handle.
    pub(crate) fn remove_mesh(&mut self, id: MeshId) {
        if let Some(slot) = self.meshes.get_mut(id.index()) {
            *slot = None;
        }
    }

    /// Releases a material's bind group without reusing its handle.
    pub(crate) fn remove_material(&mut self, id: MaterialId) {
        if let Some(slot) = self.materials.get_mut(id.index()) {
            *slot = None;
        }
    }

    /// Releases a texture view and sampler without reusing its handle.
    pub(crate) fn remove_texture(&mut self, id: TextureId) {
        if let Some(slot) = self.textures.get_mut(id.index()) {
            *slot = None;
        }
    }

    /// Uploads `image` as a texture sampled with `sampler`.
    ///
    /// `srgb` says whether the texels are sRGB-encoded colour, which the GPU
    /// then decodes to linear light when sampling.
    pub(crate) fn create_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &ImageData,
        sampler: SamplerData,
        srgb: bool,
    ) -> Result<TextureId, RenderError> {
        let expected = image.width as usize * image.height as usize * 4;
        let limit = device.limits().max_texture_dimension_2d;
        if image.width == 0 || image.height == 0 || image.pixels.len() != expected {
            return Err(RenderError::InvalidAsset(format!(
                "a {}x{} image holds {} bytes, expected {expected}",
                image.width,
                image.height,
                image.pixels.len()
            )));
        }
        if image.width > limit || image.height > limit {
            return Err(RenderError::InvalidAsset(format!(
                "a {}x{} image exceeds the device limit of {limit}",
                image.width, image.height
            )));
        }
        let texture = upload_texture(
            device,
            queue,
            &self.mipmaps,
            image,
            sampler,
            srgb,
            "texture",
        );
        self.textures.push(Some(texture));
        Ok(TextureId::from_index(self.textures.len() as u32 - 1))
    }

    /// Uploads `mesh` into vertex and index buffers.
    pub(crate) fn create_mesh(
        &mut self,
        device: &wgpu::Device,
        mesh: &MeshData,
    ) -> Result<MeshId, RenderError> {
        if mesh.positions.is_empty() || mesh.indices.is_empty() || !mesh.is_consistent() {
            return Err(RenderError::InvalidAsset(String::from(
                "a mesh is empty or its attributes and indices disagree",
            )));
        }
        let vertices: Vec<Vertex> = (0..mesh.positions.len())
            .map(|index| Vertex {
                position: mesh.positions[index].to_array(),
                normal: mesh.normals[index].to_array(),
                tangent: mesh.tangents[index].to_array(),
                uv: mesh.uvs[index].to_array(),
            })
            .collect();
        let buffer = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        self.meshes.push(Some(GpuMesh {
            vertices: buffer(
                "mesh vertices",
                bytemuck::cast_slice(&vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: buffer(
                "mesh indices",
                bytemuck::cast_slice(&mesh.indices),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: mesh.indices.len() as u32,
        }));
        Ok(MeshId::from_index(self.meshes.len() as u32 - 1))
    }

    /// Builds the bind group of `material`, whose textures must already be uploaded.
    pub(crate) fn create_material(
        &mut self,
        device: &wgpu::Device,
        material: &MaterialData<TextureId>,
    ) -> Result<MaterialId, RenderError> {
        let slots = [
            (material.base_color_texture, &self.white),
            (material.metallic_roughness_texture, &self.white),
            (material.normal_texture, &self.flat_normal),
            (material.occlusion_texture, &self.white),
            (material.emissive_texture, &self.white),
        ];
        let mut bound = Vec::with_capacity(slots.len());
        for (id, fallback) in slots {
            bound.push(match id {
                Some(id) => self
                    .textures
                    .get(id.index())
                    .and_then(Option::as_ref)
                    .ok_or_else(|| {
                        RenderError::InvalidAsset(String::from(
                            "a material uses an unknown texture",
                        ))
                    })?,
                None => fallback,
            });
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material"),
            contents: bytemuck::bytes_of(&MaterialUniform::new(material)),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }];
        entries.extend(
            bound
                .iter()
                .enumerate()
                .map(|(slot, texture)| wgpu::BindGroupEntry {
                    binding: 1 + slot as u32,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                }),
        );
        entries.extend(
            bound
                .iter()
                .enumerate()
                .map(|(slot, texture)| wgpu::BindGroupEntry {
                    binding: 6 + slot as u32,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                }),
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &self.material_layout,
            entries: &entries,
        });
        self.materials.push(Some(GpuMaterial {
            group,
            blend: matches!(material.alpha_mode, AlphaMode::Blend),
            double_sided: material.double_sided,
        }));
        Ok(MaterialId::from_index(self.materials.len() as u32 - 1))
    }
}

/// The layout of group 1: the material uniform, then five textures, then their samplers.
fn material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let uniform = wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let textures = (0..5).map(|slot| wgpu::BindGroupLayoutEntry {
        binding: 1 + slot,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    });
    let samplers = (0..5).map(|slot| wgpu::BindGroupLayoutEntry {
        binding: 6 + slot,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    });
    let entries: Vec<_> = std::iter::once(uniform)
        .chain(textures)
        .chain(samplers)
        .collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("material"),
        entries: &entries,
    })
}

/// Creates a texture holding `image` with a full mip chain, and its sampler.
fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mipmaps: &MipmapGenerator,
    image: &ImageData,
    sampler: SamplerData,
    srgb: bool,
    label: &str,
) -> GpuTexture {
    let size = wgpu::Extent3d {
        width: image.width,
        height: image.height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: mip_level_count(image.width, image.height),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        },
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &image.pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(image.width * 4),
            rows_per_image: Some(image.height),
        },
        size,
    );
    mipmaps.generate(device, queue, &texture);
    GpuTexture {
        view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
        sampler: create_sampler(device, sampler),
    }
}

/// The wgpu sampler for `sampler`, with anisotropy where every filter is linear.
fn create_sampler(device: &wgpu::Device, sampler: SamplerData) -> wgpu::Sampler {
    let filter = |filter| match filter {
        Filter::Nearest => wgpu::FilterMode::Nearest,
        Filter::Linear => wgpu::FilterMode::Linear,
    };
    let wrap = |wrap| match wrap {
        Wrap::Repeat => wgpu::AddressMode::Repeat,
        Wrap::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        Wrap::ClampToEdge => wgpu::AddressMode::ClampToEdge,
    };
    let all_linear = sampler.magnify == Filter::Linear && sampler.minify == Filter::Linear;
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("texture"),
        address_mode_u: wrap(sampler.wrap_u),
        address_mode_v: wrap(sampler.wrap_v),
        mag_filter: filter(sampler.magnify),
        min_filter: filter(sampler.minify),
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp: if all_linear { ANISOTROPY } else { 1 },
        ..Default::default()
    })
}
