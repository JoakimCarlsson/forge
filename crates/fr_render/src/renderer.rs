//! The device, the surface and the one frame each redraw presents.
//!
//! [`Renderer`] is the only thing in the workspace that holds a wgpu device.
//! Callers hand it a [`DrawList`] in logical pixels; scaling to physical
//! pixels, rasterizing glyphs and submitting the pass happen in here.

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::RenderError;
use crate::atlas::GlyphAtlas;
use crate::color::Rgba;
use crate::draw::DrawList;
use crate::geometry::{Rect, Size};
use crate::pipeline::{GlyphInstance, InstanceBuffer, QuadInstance, Viewport, build_pipeline};
use crate::text::TextSystem;

/// Owns the GPU device, queue and swapchain surface backing one window.
pub struct Renderer {
    /// The surface presented to.
    surface: wgpu::Surface<'static>,
    /// The device resources are created on.
    device: wgpu::Device,
    /// The queue work is submitted to.
    queue: wgpu::Queue,
    /// The swapchain configuration most recently requested.
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
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(Arc::new(window))
            .map_err(RenderError::Surface)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(RenderError::Adapter)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("forge device"),
            ..Default::default()
        }))
        .map_err(RenderError::Device)?;

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
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

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
            surface,
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
        })
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
    /// Zero-sized requests are ignored.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        self.scale = scale.max(f32::EPSILON);
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
    }

    /// Recreates the swapchain at the latest requested physical size.
    fn configure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
        self.configured_size = (self.config.width, self.config.height);
    }

    /// Draws one frame of `list` over `clear` and presents it.
    ///
    /// A swapchain that is outdated or lost is reconfigured and the frame
    /// skipped; one that is timed out or hidden skips the frame as well.
    pub fn render(&mut self, list: &DrawList, clear: Rgba) {
        self.text.end_frame();
        if self.configured_size != (self.config.width, self.config.height) {
            self.configure_surface();
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure_surface();
                return;
            }
            _ => return,
        };

        self.queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::bytes_of(&Viewport {
                size: [self.config.width as f32, self.config.height as f32],
                padding: [0.0; 2],
            }),
        );

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

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("forge frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("forge frame pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(clear.r),
                            g: f64::from(clear.g),
                            b: f64::from(clear.b),
                            a: f64::from(clear.a),
                        }),
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
        self.queue.present(frame);
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
                border: [quad.border_width * scale, 0.0],
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

/// Converts a logical clip rectangle to the physical bounds shaders test.
fn physical_clip(clip: Rect, scale: f32) -> [f32; 4] {
    [
        clip.left() * scale,
        clip.top() * scale,
        clip.right() * scale,
        clip.bottom() * scale,
    ]
}
