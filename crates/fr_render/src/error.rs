//! Renderer errors.

use std::fmt;

/// A failure to set up or use the graphics device.
#[derive(Debug)]
pub enum RenderError {
    /// The window surface could not be created.
    Surface(wgpu::CreateSurfaceError),
    /// No adapter can present to the surface.
    Adapter(wgpu::RequestAdapterError),
    /// The adapter refused to create a device.
    Device(wgpu::RequestDeviceError),
    /// The surface offers no configuration the adapter supports.
    Unsupported,
    /// A frame was requested from a renderer that presents to a window.
    NotOffscreen,
    /// Reading a frame back from the device failed.
    Readback(String),
    /// An asset handed to the renderer cannot be uploaded.
    InvalidAsset(String),
}

impl fmt::Display for RenderError {
    /// Writes the failure with the cause that produced it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surface(error) => write!(f, "surface creation failed: {error}"),
            Self::Adapter(error) => write!(f, "no suitable adapter: {error}"),
            Self::Device(error) => write!(f, "device creation failed: {error}"),
            Self::Unsupported => write!(f, "the surface supports no usable configuration"),
            Self::NotOffscreen => write!(f, "only an offscreen renderer can capture a frame"),
            Self::Readback(reason) => write!(f, "reading the frame back failed: {reason}"),
            Self::InvalidAsset(reason) => write!(f, "invalid asset: {reason}"),
        }
    }
}

impl std::error::Error for RenderError {
    /// The underlying wgpu error, when there is one.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Surface(error) => Some(error),
            Self::Adapter(error) => Some(error),
            Self::Device(error) => Some(error),
            Self::Unsupported | Self::NotOffscreen | Self::Readback(_) | Self::InvalidAsset(_) => {
                None
            }
        }
    }
}
