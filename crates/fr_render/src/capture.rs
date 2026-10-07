//! Reading a finished offscreen frame back to the CPU.

use crate::RenderError;

/// The bytes per pixel of the offscreen format.
const BYTES_PER_PIXEL: u32 = 4;

/// A frame read back from an offscreen renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    /// The width in physical pixels.
    pub width: u32,
    /// The height in physical pixels.
    pub height: u32,
    /// The pixels as straight RGBA, 8 bits per channel, top row first and
    /// without row padding: `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// The bytes one row of `width` pixels takes in a buffer a texture is copied to.
fn padded_row_bytes(width: u32) -> u32 {
    (width * BYTES_PER_PIXEL).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
}

/// Copies `texture`, an RGBA8 texture of `width` by `height` pixels created
/// with copy-source usage, to the CPU and waits for it.
///
/// # Errors
///
/// Returns [`RenderError::Readback`] when the device cannot map the staging
/// buffer.
pub(crate) fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Capture, RenderError> {
    let padded = padded_row_bytes(width);
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture staging"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("capture"),
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let (sender, receiver) = std::sync::mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| RenderError::Readback(error.to_string()))?;
    receiver
        .recv()
        .map_err(|error| RenderError::Readback(error.to_string()))?
        .map_err(|error| RenderError::Readback(error.to_string()))?;

    let mapped = staging
        .slice(..)
        .get_mapped_range()
        .map_err(|error| RenderError::Readback(error.to_string()))?;
    let row_bytes = (width * BYTES_PER_PIXEL) as usize;
    let mut rgba = Vec::with_capacity(row_bytes * height as usize);
    for row in mapped.chunks_exact(padded as usize) {
        rgba.extend_from_slice(&row[..row_bytes]);
    }
    drop(mapped);
    staging.unmap();
    Ok(Capture {
        width,
        height,
        rgba,
    })
}
