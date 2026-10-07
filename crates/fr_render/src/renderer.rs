//! The device and swapchain.

use std::sync::Arc;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::RenderError;

/// Owns the device, the queue and the swapchain of one window.
pub struct Renderer {
    /// The surface presented to.
    surface: wgpu::Surface<'static>,
    /// The device resources are created on.
    device: wgpu::Device,
    /// The queue work is submitted to.
    queue: wgpu::Queue,
    /// The current swapchain configuration.
    config: wgpu::SurfaceConfiguration,
}

impl Renderer {
    /// Creates a renderer that presents to `window`, whose drawable area is
    /// `width` by `height` physical pixels.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] when the surface, adapter or device cannot be created.
    pub fn new<W>(window: W, width: u32, height: u32) -> Result<Self, RenderError>
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
        let config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or(RenderError::Unsupported)?;
        surface.configure(&device, &config);
        Ok(Self {
            surface,
            device,
            queue,
            config,
        })
    }

    /// Reconfigures the swapchain for a new size; zero-sized requests are ignored.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Clears the window to `color` and presents it.
    ///
    /// A swapchain that is outdated or lost is reconfigured and the frame skipped.
    pub fn clear(&mut self, color: [f64; 4]) {
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("forge clear"),
            });
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("forge clear pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: color[0],
                        g: color[1],
                        b: color[2],
                        a: color[3],
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        }));
        self.queue.submit([encoder.finish()]);
        self.queue.present(texture);
    }
}
