//! Loading and uploading assets, handed to the app once the renderer exists.

use std::fmt;
use std::path::Path;

use fr_assets::{AssetError, ImageData, MaterialData, MeshData, SamplerData};
use fr_core::{MaterialId, MeshId, TextureId};
use fr_render::{Model, RenderError, Renderer};

/// A failure to load an asset and put it on the graphics device.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be read or understood.
    Asset(AssetError),
    /// The graphics device rejected what the file contained.
    Render(RenderError),
}

impl fmt::Display for LoadError {
    /// Writes the failure with the cause that produced it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => write!(f, "{error}"),
            Self::Render(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for LoadError {
    /// The underlying asset or render error.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            Self::Render(error) => Some(error),
        }
    }
}

impl From<AssetError> for LoadError {
    /// Wraps an import failure.
    fn from(error: AssetError) -> Self {
        Self::Asset(error)
    }
}

impl From<RenderError> for LoadError {
    /// Wraps an upload failure.
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}

/// Uploads meshes, materials, textures and model files, returning the handles
/// a [`crate::Scene`] draws with.
pub struct Assets<'a> {
    /// The renderer uploads go to.
    renderer: &'a mut Renderer,
}

impl<'a> Assets<'a> {
    /// Wraps `renderer` for the duration of a loading hook.
    pub(crate) fn new(renderer: &'a mut Renderer) -> Self {
        Self { renderer }
    }

    /// Reads the glTF or GLB file at `path` and uploads everything in it.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError`] when the file cannot be read or parsed, or when
    /// its contents cannot be uploaded.
    pub fn load_gltf(&mut self, path: impl AsRef<Path>) -> Result<Model, LoadError> {
        let data = fr_assets::load_gltf(path)?;
        Ok(self.renderer.upload_model(&data)?)
    }

    /// Uploads `mesh`.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError`] when the mesh is empty or inconsistent.
    pub fn add_mesh(&mut self, mesh: &MeshData) -> Result<MeshId, LoadError> {
        Ok(self.renderer.create_mesh(mesh)?)
    }

    /// Uploads `image` as a texture; set `srgb` for colour images.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError`] when the image is malformed or too large.
    pub fn add_texture(
        &mut self,
        image: &ImageData,
        sampler: SamplerData,
        srgb: bool,
    ) -> Result<TextureId, LoadError> {
        Ok(self.renderer.create_texture(image, sampler, srgb)?)
    }

    /// Creates a material from factors and uploaded textures.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError`] when a texture handle is unknown.
    pub fn add_material(
        &mut self,
        material: &MaterialData<TextureId>,
    ) -> Result<MaterialId, LoadError> {
        Ok(self.renderer.create_material(material)?)
    }

    /// The material for anything without one of its own: white and half rough.
    pub fn default_material(&self) -> MaterialId {
        self.renderer.default_material()
    }
}
