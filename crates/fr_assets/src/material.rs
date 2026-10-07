//! Metallic-roughness material factors and texture slots.

use fr_core::{Vec3, Vec4, srgb_to_linear};

/// How a material's alpha is interpreted.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AlphaMode {
    /// Alpha is ignored.
    #[default]
    Opaque,
    /// Fragments with alpha below the cutoff are discarded.
    Mask(f32),
    /// Alpha blends the surface with what is behind it.
    Blend,
}

/// A metallic-roughness PBR material whose texture slots hold `T`.
///
/// A loader fills the slots with indices into the file's textures, a renderer
/// with handles to uploaded ones; [`MaterialData::try_map_textures`] converts one
/// to the other. Colours and factors are in linear light.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialData<T> {
    /// The name given in the source, when it has one.
    pub name: Option<String>,
    /// The linear base colour and opacity, multiplied with the base colour texture.
    pub base_color: Vec4,
    /// The metalness in `0..=1`, multiplied with the blue channel of the metal-rough texture.
    pub metallic: f32,
    /// The roughness in `0..=1`, multiplied with the green channel of the metal-rough texture.
    pub roughness: f32,
    /// The linear emitted colour, multiplied with the emissive texture.
    pub emissive: Vec3,
    /// How strongly the normal texture bends the surface normal.
    pub normal_scale: f32,
    /// How strongly the occlusion texture darkens ambient light.
    pub occlusion_strength: f32,
    /// How alpha is interpreted.
    pub alpha_mode: AlphaMode,
    /// Whether back faces are drawn, lit as seen from behind.
    pub double_sided: bool,
    /// The sRGB base colour texture.
    pub base_color_texture: Option<T>,
    /// The linear texture holding roughness in green and metalness in blue.
    pub metallic_roughness_texture: Option<T>,
    /// The linear tangent-space normal texture.
    pub normal_texture: Option<T>,
    /// The linear texture holding ambient occlusion in red.
    pub occlusion_texture: Option<T>,
    /// The sRGB emissive texture.
    pub emissive_texture: Option<T>,
}

impl<T> MaterialData<T> {
    /// A dielectric-to-metal surface of one colour, with no textures.
    ///
    /// `base_srgb` is the colour as an artist writes it, with each channel in
    /// `0..=1`; it is converted to linear light here.
    pub fn solid(base_srgb: [f32; 3], metallic: f32, roughness: f32) -> Self {
        Self {
            base_color: Vec4::new(
                srgb_to_linear(base_srgb[0]),
                srgb_to_linear(base_srgb[1]),
                srgb_to_linear(base_srgb[2]),
                1.0,
            ),
            metallic,
            roughness,
            ..Self::default()
        }
    }

    /// This material with every texture slot converted by `map`, or the first
    /// error `map` returns.
    pub fn try_map_textures<U, E>(
        &self,
        mut map: impl FnMut(&T, TextureSlot) -> Result<U, E>,
    ) -> Result<MaterialData<U>, E> {
        let mut slot =
            |texture: &Option<T>, slot| texture.as_ref().map(|t| map(t, slot)).transpose();
        Ok(MaterialData {
            name: self.name.clone(),
            base_color: self.base_color,
            metallic: self.metallic,
            roughness: self.roughness,
            emissive: self.emissive,
            normal_scale: self.normal_scale,
            occlusion_strength: self.occlusion_strength,
            alpha_mode: self.alpha_mode,
            double_sided: self.double_sided,
            base_color_texture: slot(&self.base_color_texture, TextureSlot::BaseColor)?,
            metallic_roughness_texture: slot(
                &self.metallic_roughness_texture,
                TextureSlot::MetallicRoughness,
            )?,
            normal_texture: slot(&self.normal_texture, TextureSlot::Normal)?,
            occlusion_texture: slot(&self.occlusion_texture, TextureSlot::Occlusion)?,
            emissive_texture: slot(&self.emissive_texture, TextureSlot::Emissive)?,
        })
    }
}

/// Which texture slot of a material is being converted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextureSlot {
    /// The base colour slot, holding sRGB colour.
    BaseColor,
    /// The metallic-roughness slot, holding linear data.
    MetallicRoughness,
    /// The normal slot, holding linear data.
    Normal,
    /// The occlusion slot, holding linear data.
    Occlusion,
    /// The emissive slot, holding sRGB colour.
    Emissive,
}

impl TextureSlot {
    /// Whether the slot's texels are sRGB-encoded colour rather than linear data.
    pub const fn is_srgb(self) -> bool {
        matches!(self, Self::BaseColor | Self::Emissive)
    }
}

impl<T> Default for MaterialData<T> {
    /// A white, non-metallic, half-rough, opaque, single-sided surface.
    fn default() -> Self {
        Self {
            name: None,
            base_color: Vec4::ONE,
            metallic: 0.0,
            roughness: 0.5,
            emissive: Vec3::ZERO,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
            base_color_texture: None,
            metallic_roughness_texture: None,
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
        }
    }
}
