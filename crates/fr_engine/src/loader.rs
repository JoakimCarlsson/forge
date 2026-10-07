//! Loading and uploading assets, handed to the app once the renderer exists.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use fr_assets::{AssetError, ModelData};
use fr_image::{ImageData, SamplerData, TextureId};
use fr_material::{MaterialData, MaterialId, TextureSlot};
use fr_mesh::{MeshData, MeshId};
use fr_render::{MeshInstance, Model, RenderError, Renderer};

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
        Ok(self.upload_model(&data)?)
    }

    /// Uploads every texture, material and mesh of `model` and returns the placed meshes as
    /// a [`Model`].
    ///
    /// A texture used both as colour and as data is uploaded once for each.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::InvalidAsset`] when the model refers to an image, texture,
    /// material or mesh it does not contain, or when one of them is invalid.
    fn upload_model(&mut self, model: &ModelData) -> Result<Model, RenderError> {
        let mut uploaded: HashMap<(usize, bool), TextureId> = HashMap::new();
        let mut materials = Vec::with_capacity(model.materials.len());
        for material in &model.materials {
            let resolved = material.try_map_textures(|&texture, slot| {
                self.upload_model_texture(model, texture, slot, &mut uploaded)
            })?;
            materials.push(self.renderer.create_material(&resolved)?);
        }
        let meshes = model
            .meshes
            .iter()
            .map(|mesh| self.renderer.create_mesh(mesh))
            .collect::<Result<Vec<_>, _>>()?;
        let mut instances = Vec::with_capacity(model.parts.len());
        for part in &model.parts {
            instances.push(MeshInstance {
                mesh: pick_item(&meshes, part.mesh)?,
                material: match part.material {
                    Some(index) => pick_item(&materials, index)?,
                    None => self.renderer.default_material(),
                },
                transform: part.transform,
            });
        }
        Ok(Model {
            instances,
            lights: model.lights.clone(),
        })
    }

    /// The texture `index` of `model` uploaded for `slot`, reusing an earlier upload.
    fn upload_model_texture(
        &mut self,
        model: &ModelData,
        index: usize,
        slot: TextureSlot,
        uploaded: &mut HashMap<(usize, bool), TextureId>,
    ) -> Result<TextureId, RenderError> {
        let key = (index, slot.is_srgb());
        if let Some(&id) = uploaded.get(&key) {
            return Ok(id);
        }
        let missing =
            || RenderError::InvalidAsset(String::from("a material uses a missing texture"));
        let texture = model.textures.get(index).ok_or_else(missing)?;
        let image = model.images.get(texture.image).ok_or_else(missing)?;
        let id = self
            .renderer
            .create_texture(image, texture.sampler, slot.is_srgb())?;
        uploaded.insert(key, id);
        Ok(id)
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

/// The item at `index` of a table of uploaded handles.
fn pick_item<T: Copy>(table: &[T], index: usize) -> Result<T, RenderError> {
    table.get(index).copied().ok_or_else(|| {
        RenderError::InvalidAsset(format!("a part refers to the missing item {index}"))
    })
}
