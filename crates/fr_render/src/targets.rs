//! The depth buffer and multisampled colour target the 3D pass renders into.

/// The format of the depth buffer.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Depth and, when multisampling, colour textures matching the swapchain size.
pub(crate) struct SceneTargets {
    /// The physical size the textures were created at.
    size: (u32, u32),
    /// The depth view.
    depth: wgpu::TextureView,
    /// The multisampled colour view that resolves into the swapchain, if any.
    color: Option<wgpu::TextureView>,
}

impl SceneTargets {
    /// Creates targets of `size` physical pixels, multisampled `samples` times
    /// in `format` when `samples` is above one.
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        samples: u32,
    ) -> Self {
        let create = |label, format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0.max(1),
                        height: size.1.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        Self {
            size,
            depth: create(
                "scene depth",
                DEPTH_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            color: (samples > 1).then(|| {
                create(
                    "scene color",
                    format,
                    wgpu::TextureUsages::RENDER_ATTACHMENT,
                )
            }),
        }
    }

    /// Whether the targets are already `size` physical pixels.
    pub(crate) fn matches(&self, size: (u32, u32)) -> bool {
        self.size == size
    }

    /// The depth view.
    pub(crate) fn depth(&self) -> &wgpu::TextureView {
        &self.depth
    }

    /// The multisampled colour view, when multisampling.
    pub(crate) fn color(&self) -> Option<&wgpu::TextureView> {
        self.color.as_ref()
    }
}
