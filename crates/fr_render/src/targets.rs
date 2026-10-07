//! The depth buffer and multisampled colour target the 3D pass renders into.

/// The format of the depth buffer.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A rectangle of the render target in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PixelRect {
    /// The left edge.
    pub(crate) x: u32,
    /// The top edge.
    pub(crate) y: u32,
    /// The width, at least one.
    pub(crate) width: u32,
    /// The height, at least one.
    pub(crate) height: u32,
}

impl PixelRect {
    /// The whole target of `size` physical pixels.
    pub(crate) fn full(size: (u32, u32)) -> Self {
        Self {
            x: 0,
            y: 0,
            width: size.0.max(1),
            height: size.1.max(1),
        }
    }

    /// The part of a `target`-sized area that the logical rectangle `rect`
    /// covers at `scale` physical pixels per logical pixel, or none when
    /// nothing of it is inside.
    pub(crate) fn clamped(rect: fr_math::Rect, scale: f32, target: (u32, u32)) -> Option<Self> {
        let edge = |value: f32, limit: u32| (value * scale).round().clamp(0.0, limit as f32) as u32;
        let left = edge(rect.left(), target.0);
        let top = edge(rect.top(), target.1);
        let right = edge(rect.right(), target.0);
        let bottom = edge(rect.bottom(), target.1);
        (right > left && bottom > top).then_some(Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }

    /// Width over height.
    pub(crate) fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

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
