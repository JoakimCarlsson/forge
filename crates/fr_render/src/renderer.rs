//! The device, the surface and the one frame each redraw presents.
//!
//! [`Renderer`] is the only thing in the workspace that holds a wgpu device.
//! Callers hand it a [`DrawList`] in logical pixels; scaling to physical
//! pixels, rasterizing glyphs and submitting the passes happen in here. A 3D
//! [`Scene`] drawn first and the UI drawn over it share the one frame.

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use fr_color::Rgba;
use fr_image::{ImageData, SamplerData, TextureId};
use fr_material::{MaterialData, MaterialId};
use fr_math::{Rect, Size};
use fr_mesh::{MeshData, MeshId};

use crate::RenderError;
use crate::atlas::GlyphAtlas;
use crate::capture::{Capture, read_texture};
use crate::draw::DrawList;
use crate::forward::ForwardPass;
use crate::pipeline::{GlyphInstance, InstanceBuffer, QuadInstance, Viewport, build_pipeline};
use crate::resources::Resources;
use crate::scene::Scene;
use crate::targets::PixelRect;
use crate::text::TextSystem;

/// The multisample count the 3D pass uses where the surface supports it.
const SAMPLES: u32 = 4;

/// The colour format an offscreen renderer draws into: 8 bits a channel and
/// not sRGB, like the swapchain format the window path prefers.
const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Owns the GPU device and queue, and either the swapchain surface backing one
/// window or the texture an offscreen renderer draws into.
pub struct Renderer {
    /// The surface presented to, or none when rendering offscreen.
    surface: Option<wgpu::Surface<'static>>,
    /// The texture frames are drawn into when there is no surface.
    offscreen: Option<wgpu::Texture>,
    /// The device resources are created on.
    device: wgpu::Device,
    /// The queue work is submitted to.
    queue: wgpu::Queue,
    /// The present modes the surface supports.
    present_modes: Vec<wgpu::PresentMode>,
    /// The swapchain configuration most recently requested; offscreen only its size and format are used.
    config: wgpu::SurfaceConfiguration,
    /// The physical size the swapchain was last configured to present.
    configured_size: (u32, u32),
    /// Physical pixels per logical pixel.
    scale: f32,
    /// Shapes text for layout and for drawing.
    text: TextSystem,
    /// The shared coverage texture glyphs are drawn from.
    atlas: GlyphAtlas,
    /// The uniform holding the viewport size.
    viewport_buffer: wgpu::Buffer,
    /// The viewport uniform as the pipelines' first group.
    viewport_group: wgpu::BindGroup,
    /// Draws rounded, bordered rectangles.
    quad_pipeline: wgpu::RenderPipeline,
    /// The quad instances of the frame being drawn.
    quad_instances: InstanceBuffer,
    /// Draws glyphs from the atlas.
    glyph_pipeline: wgpu::RenderPipeline,
    /// The glyph instances of the frame being drawn.
    glyph_instances: InstanceBuffer,
    /// Quad instances gathered for a frame, reused from frame to frame.
    quads: Vec<QuadInstance>,
    /// Glyph instances gathered for a frame, reused from frame to frame.
    glyphs: Vec<GlyphInstance>,
    /// The meshes, materials and textures the 3D pass draws from.
    resources: Resources,
    /// The 3D pass.
    forward: ForwardPass,
    /// The material of anything drawn without one of its own.
    default_material: MaterialId,
}

impl Renderer {
    /// Creates a renderer that presents to `window`, whose drawable area is
    /// `width` by `height` physical pixels at `scale` physical pixels per
    /// logical pixel.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] when the surface, adapter or device cannot be created.
    pub fn new<W>(window: W, width: u32, height: u32, scale: f32) -> Result<Self, RenderError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let instance = new_instance();
        let surface = instance
            .create_surface(Arc::new(window))
            .map_err(RenderError::Surface)?;
        let (adapter, device, queue) = request_device(&instance, Some(&surface))?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or(RenderError::Unsupported)?;
        let alpha_mode = capabilities
            .alpha_modes
            .first()
            .copied()
            .ok_or(RenderError::Unsupported)?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: pick_present_mode(&capabilities.present_modes, false),
            alpha_mode,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let samples = sample_count(&adapter, format);
        Self::assemble(
            device,
            queue,
            Some(surface),
            capabilities.present_modes,
            config,
            samples,
            scale,
        )
    }

    /// Creates a renderer with no window: frames are drawn into a texture of
    /// `width` by `height` physical pixels at `scale` physical pixels per
    /// logical pixel, and read back with [`Self::capture`].
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] when no adapter or device can be created.
    pub fn new_offscreen(width: u32, height: u32, scale: f32) -> Result<Self, RenderError> {
        let instance = new_instance();
        let (adapter, device, queue) = request_device(&instance, None)?;
        let format = OFFSCREEN_FORMAT;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        let samples = sample_count(&adapter, format);
        let mut renderer = Self::assemble(device, queue, None, Vec::new(), config, samples, scale)?;
        renderer.recreate_offscreen();
        Ok(renderer)
    }

    /// Builds everything that does not depend on whether frames go to a
    /// surface or a texture, around the device and the target's `config`.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::InvalidAsset`] when the default material cannot be created.
    fn assemble(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        present_modes: Vec<wgpu::PresentMode>,
        config: wgpu::SurfaceConfiguration,
        samples: u32,
        scale: f32,
    ) -> Result<Self, RenderError> {
        let format = config.format;
        let mut resources = Resources::new(&device, &queue);
        let default_material = resources.create_material(&device, &MaterialData::default())?;
        let forward = ForwardPass::new(
            &device,
            &resources,
            format,
            (config.width, config.height),
            samples,
        );

        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewport"),
            size: size_of::<Viewport>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let viewport_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport"),
            layout: &viewport_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        let atlas = GlyphAtlas::new(&device);
        let quad_pipeline = build_pipeline(
            &device,
            format,
            "quad",
            include_str!("shaders/quad.wgsl"),
            &[Some(&viewport_layout)],
            size_of::<QuadInstance>() as u64,
            &QuadInstance::ATTRIBUTES,
        );
        let glyph_pipeline = build_pipeline(
            &device,
            format,
            "glyph",
            include_str!("shaders/glyph.wgsl"),
            &[Some(&viewport_layout), Some(atlas.layout())],
            size_of::<GlyphInstance>() as u64,
            &GlyphInstance::ATTRIBUTES,
        );

        Ok(Self {
            present_modes,
            surface,
            offscreen: None,
            device,
            queue,
            configured_size: (config.width, config.height),
            config,
            scale: scale.max(f32::EPSILON),
            text: TextSystem::new(),
            atlas,
            viewport_buffer,
            viewport_group,
            quad_pipeline,
            quad_instances: InstanceBuffer::new("quad instances"),
            glyph_pipeline,
            glyph_instances: InstanceBuffer::new("glyph instances"),
            quads: Vec::new(),
            glyphs: Vec::new(),
            resources,
            forward,
            default_material,
        })
    }

    /// The material meshes get when they are drawn without one of their own:
    /// white, non-metallic and half rough.
    pub fn default_material(&self) -> MaterialId {
        self.default_material
    }

    /// Uploads `image` as a texture read with `sampler`.
    ///
    /// Set `srgb` for colour images and clear it for data such as normals and
    /// roughness, so sampling returns linear values either way.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::InvalidAsset`] when the pixels do not match the
    /// size or the size exceeds the device's limit.
    pub fn create_texture(
        &mut self,
        image: &ImageData,
        sampler: SamplerData,
        srgb: bool,
    ) -> Result<TextureId, RenderError> {
        self.resources
            .create_texture(&self.device, &self.queue, image, sampler, srgb)
    }

    /// Uploads `mesh` and returns the handle to draw it with.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::InvalidAsset`] when the mesh is empty or its
    /// attributes and indices disagree.
    pub fn create_mesh(&mut self, mesh: &MeshData) -> Result<MeshId, RenderError> {
        self.resources.create_mesh(&self.device, mesh)
    }

    /// Creates a material from factors and uploaded textures.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::InvalidAsset`] when a texture handle is unknown.
    pub fn create_material(
        &mut self,
        material: &MaterialData<TextureId>,
    ) -> Result<MaterialId, RenderError> {
        self.resources.create_material(&self.device, material)
    }

    /// The text system layout measures with and draw lists shape through.
    pub fn text(&mut self) -> &mut TextSystem {
        &mut self.text
    }

    /// The surface size in logical pixels.
    pub fn size(&self) -> Size {
        Size::new(
            self.config.width as f32 / self.scale,
            self.config.height as f32 / self.scale,
        )
    }

    /// Records the latest physical size and scale factor for the next frame.
    ///
    /// Resize events can arrive faster than frames are presented, so the
    /// swapchain is only reconfigured when a frame needs the final size.
    /// An offscreen renderer recreates its target texture at once, dropping
    /// the last frame. Zero-sized requests are ignored.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        self.scale = scale.max(f32::EPSILON);
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        if self.surface.is_none() {
            self.recreate_offscreen();
        }
    }

    /// Chooses whether presenting waits for the display's refresh.
    ///
    /// Off, the default, presents as fast as frames finish: immediate where the
    /// surface offers it, else mailbox, else the vsynced queue every surface has.
    pub fn set_vsync(&mut self, enabled: bool) {
        let mode = pick_present_mode(&self.present_modes, enabled);
        if mode != self.config.present_mode {
            self.config.present_mode = mode;
            self.configure_surface();
        }
    }

    /// Whether presenting waits for the display's refresh.
    pub fn vsync(&self) -> bool {
        matches!(
            self.config.present_mode,
            wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed | wgpu::PresentMode::AutoVsync
        )
    }

    /// Recreates the swapchain at the latest requested physical size.
    fn configure_surface(&mut self) {
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        self.configured_size = (self.config.width, self.config.height);
    }

    /// Recreates the offscreen target texture at the configured size.
    fn recreate_offscreen(&mut self) {
        self.offscreen = Some(self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen target"),
            size: wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: self.config.usage,
            view_formats: &[],
        }));
        self.configured_size = (self.config.width, self.config.height);
    }

    /// Reads back the last frame an offscreen renderer drew.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::NotOffscreen`] when the renderer presents to a
    /// window, and [`RenderError::Readback`] when the device cannot hand the
    /// pixels over.
    pub fn capture(&mut self) -> Result<Capture, RenderError> {
        let texture = match (&self.surface, &self.offscreen) {
            (None, Some(texture)) => texture,
            _ => return Err(RenderError::NotOffscreen),
        };
        read_texture(
            &self.device,
            &self.queue,
            texture,
            self.config.width,
            self.config.height,
        )
    }

    /// Gets the view the next frame is drawn into, and the swapchain frame to
    /// present afterwards when there is one.
    ///
    /// A swapchain that is outdated or lost is reconfigured and none is
    /// returned; one that is timed out or hidden returns none as well.
    fn acquire(&mut self) -> Option<(wgpu::TextureView, Option<wgpu::SurfaceTexture>)> {
        if self.configured_size != (self.config.width, self.config.height) {
            self.configure_surface();
        }
        let Some(surface) = &self.surface else {
            let view = self
                .offscreen
                .as_ref()?
                .create_view(&wgpu::TextureViewDescriptor::default());
            return Some((view, None));
        };
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure_surface();
                return None;
            }
            _ => return None,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Some((view, Some(frame)))
    }

    /// Draws one frame and presents it: the 3D `scene`, if it has any
    /// instances, over `clear`, then `list` on top, in a single submission.
    ///
    /// A swapchain that is outdated or lost is reconfigured and the frame
    /// skipped; one that is timed out or hidden skips the frame as well. An
    /// offscreen renderer draws into its texture and presents nothing.
    pub fn render(&mut self, list: &DrawList, scene: Option<&Scene>, clear: Rgba) {
        self.render_in(list, scene, None, clear);
    }

    /// Draws one frame like [`Self::render`], with the 3D `scene` confined to
    /// `scene_rect`, a rectangle in logical pixels, when one is given.
    ///
    /// The camera's aspect ratio and the shadow fit follow the rectangle, which
    /// is clamped to the target; the target is still cleared to `clear`
    /// everywhere and the UI is drawn over all of it. A rectangle with nothing
    /// inside the target draws no 3D.
    pub fn render_in(
        &mut self,
        list: &DrawList,
        scene: Option<&Scene>,
        scene_rect: Option<Rect>,
        clear: Rgba,
    ) {
        self.text.end_frame();
        let Some((view, frame)) = self.acquire() else {
            return;
        };

        self.queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::bytes_of(&Viewport {
                size: [self.config.width as f32, self.config.height as f32],
                padding: [0.0; 2],
            }),
        );

        let target = (self.config.width, self.config.height);
        let viewport = match scene_rect {
            Some(rect) => PixelRect::clamped(rect, self.scale, target),
            None => Some(PixelRect::full(target)),
        };
        let scene = scene
            .filter(|scene| !scene.instances.is_empty())
            .zip(viewport);
        if let Some((scene, viewport)) = scene {
            self.forward.prepare(
                &self.device,
                &self.queue,
                &self.resources,
                scene,
                target,
                viewport,
            );
        }

        self.build_quads(list);
        self.build_glyphs(list);
        if self.atlas.overflowed() {
            self.atlas.make_room(&self.device);
            self.build_glyphs(list);
        }
        self.quad_instances
            .upload(&self.device, &self.queue, bytemuck::cast_slice(&self.quads));
        self.glyph_instances.upload(
            &self.device,
            &self.queue,
            bytemuck::cast_slice(&self.glyphs),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("forge frame"),
            });
        let clear_color = wgpu::Color {
            r: f64::from(clear.r),
            g: f64::from(clear.g),
            b: f64::from(clear.b),
            a: f64::from(clear.a),
        };
        let ui_load = if scene.is_some() {
            self.forward
                .encode(&mut encoder, &view, clear_color, &self.resources);
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(clear_color)
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("forge frame pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: ui_load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, &self.viewport_group, &[]);
            if let Some(buffer) = self.quad_instances.buffer()
                && !self.quads.is_empty()
            {
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..self.quads.len() as u32);
            }
            if let Some(buffer) = self.glyph_instances.buffer()
                && !self.glyphs.is_empty()
            {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(1, self.atlas.group(), &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..self.glyphs.len() as u32);
            }
        }
        self.queue.submit([encoder.finish()]);
        if let Some(frame) = frame {
            self.queue.present(frame);
        }
    }

    /// Converts the list's quads to physical-pixel instances.
    fn build_quads(&mut self, list: &DrawList) {
        self.quads.clear();
        let scale = self.scale;
        for (quad, clip) in list.quads() {
            self.quads.push(QuadInstance {
                origin: [quad.bounds.left() * scale, quad.bounds.top() * scale],
                size: [
                    quad.bounds.size.width * scale,
                    quad.bounds.size.height * scale,
                ],
                background: quad.background.to_array(),
                border_color: quad.border_color.to_array(),
                radii: quad.corner_radii.map(|radius| radius * scale),
                border: [quad.border_width * scale, quad.rotation],
                clip: physical_clip(*clip, scale),
            });
        }
    }

    /// Rasterizes the list's text and converts it to glyph instances, starting
    /// the batch over.
    ///
    /// When the atlas fills part way through, the batch is missing glyphs;
    /// the caller makes room in the atlas and builds it again.
    fn build_glyphs(&mut self, list: &DrawList) {
        self.glyphs.clear();
        let scale = self.scale;
        let atlas_size = self.atlas.size();
        for (text, clip) in list.texts() {
            let clip = physical_clip(*clip, scale);
            let color = text.color.to_array();
            let offset = (
                (text.origin.x * scale).round(),
                ((text.origin.y + text.run.baseline) * scale).round(),
            );
            for glyph in &text.run.glyphs {
                let physical = glyph.physical(offset, scale);
                let Some(slot) = self
                    .atlas
                    .slot(&mut self.text, &self.queue, physical.cache_key)
                else {
                    continue;
                };
                self.glyphs.push(GlyphInstance {
                    origin: [
                        (physical.x + slot.left) as f32,
                        (physical.y - slot.top) as f32,
                    ],
                    size: [slot.width as f32, slot.height as f32],
                    uv_origin: [slot.x as f32 / atlas_size, slot.y as f32 / atlas_size],
                    uv_size: [
                        slot.width as f32 / atlas_size,
                        slot.height as f32 / atlas_size,
                    ],
                    color,
                    clip,
                    rotation: [0.0; 4],
                });
            }
        }
        self.build_icons(list);
    }

    /// Rasterizes the list's icons and appends them to the glyph instances.
    ///
    /// An icon is a glyph as far as the GPU is concerned: the same atlas, the
    /// same pipeline, the same tint. What differs is only where the coverage
    /// came from, which the atlas has already forgotten by this point.
    fn build_icons(&mut self, list: &DrawList) {
        let scale = self.scale;
        let atlas_size = self.atlas.size();
        for (icon, clip) in list.icons() {
            let side = (icon.bounds.size.width.min(icon.bounds.size.height) * scale).round();
            let Some(slot) = self.atlas.icon_slot(&self.queue, icon.svg, side as u32) else {
                continue;
            };
            self.glyphs.push(GlyphInstance {
                origin: [
                    (icon.bounds.left() * scale).round(),
                    (icon.bounds.top() * scale).round(),
                ],
                size: [slot.width as f32, slot.height as f32],
                uv_origin: [slot.x as f32 / atlas_size, slot.y as f32 / atlas_size],
                uv_size: [
                    slot.width as f32 / atlas_size,
                    slot.height as f32 / atlas_size,
                ],
                color: icon.color.to_array(),
                clip: physical_clip(*clip, scale),
                rotation: [icon.rotation, 0.0, 0.0, 0.0],
            });
        }
    }
}

/// The present mode for `vsync` among the `supported` ones: the vsynced queue
/// when on, otherwise immediate, else mailbox, else the vsynced queue again.
fn pick_present_mode(supported: &[wgpu::PresentMode], vsync: bool) -> wgpu::PresentMode {
    let preferred: &[wgpu::PresentMode] = if vsync {
        &[]
    } else {
        &[wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox]
    };
    preferred
        .iter()
        .copied()
        .find(|mode| supported.contains(mode))
        .unwrap_or(wgpu::PresentMode::Fifo)
}

/// Converts a logical clip rectangle to the physical bounds shaders test.
fn physical_clip(clip: Rect, scale: f32) -> [f32; 4] {
    [
        clip.left() * scale,
        clip.top() * scale,
        clip.right() * scale,
        clip.bottom() * scale,
    ]
}

/// Creates the wgpu instance, honouring the environment such as `WGPU_VALIDATION`.
fn new_instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env())
}

/// Requests an adapter, compatible with `surface` when there is one, and a device on it.
///
/// # Errors
///
/// Returns [`RenderError::Adapter`] or [`RenderError::Device`] when either is refused.
fn request_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), RenderError> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: surface,
        ..Default::default()
    }))
    .map_err(RenderError::Adapter)?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("forge device"),
        ..Default::default()
    }))
    .map_err(RenderError::Device)?;
    Ok((adapter, device, queue))
}

/// The multisample count the 3D pass uses for `format` on `adapter`: [`SAMPLES`]
/// where supported, else one.
fn sample_count(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> u32 {
    if adapter
        .get_texture_format_features(format)
        .flags
        .sample_count_supported(SAMPLES)
    {
        SAMPLES
    } else {
        1
    }
}
