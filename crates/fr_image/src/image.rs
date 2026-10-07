//! Pixels and the sampling state a texture is read with.

use fr_handle::Handle;

/// A texture uploaded to the renderer.
pub type TextureId = Handle<TextureData>;

/// A decoded image, always four 8-bit channels per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageData {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// Row-major RGBA bytes, `width * height * 4` of them.
    pub pixels: Vec<u8>,
}

impl ImageData {
    /// A single-pixel image of one RGBA colour.
    pub fn solid(rgba: [u8; 4]) -> Self {
        Self {
            width: 1,
            height: 1,
            pixels: rgba.to_vec(),
        }
    }
}

/// How neighbouring texels are combined.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Filter {
    /// The nearest texel.
    Nearest,
    /// A blend of the surrounding texels.
    #[default]
    Linear,
}

/// What a texture coordinate outside `0..=1` reads.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Wrap {
    /// The texture tiles.
    #[default]
    Repeat,
    /// The texture tiles, every other copy flipped.
    MirroredRepeat,
    /// The edge texel stretches out.
    ClampToEdge,
}

/// How a texture is magnified, minified and wrapped.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SamplerData {
    /// The filter when the texture is magnified.
    pub magnify: Filter,
    /// The filter when the texture is minified.
    pub minify: Filter,
    /// The wrapping along the horizontal texture axis.
    pub wrap_u: Wrap,
    /// The wrapping along the vertical texture axis.
    pub wrap_v: Wrap,
}

/// An image together with the sampler it is read with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextureData {
    /// The index of the image in the owning model's images.
    pub image: usize,
    /// How the image is sampled.
    pub sampler: SamplerData,
}
