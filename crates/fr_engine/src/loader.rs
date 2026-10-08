//! CPU asset ownership and synchronous loading, available throughout execution.

use fr_app::ecs as bevy_ecs;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::Path;

use fr_app::ecs::resource::Resource;
use fr_assets::{AssetError, ModelData};
use fr_image::{ImageData, SamplerData, TextureId};
use fr_material::{MaterialData, MaterialId, TextureSlot};
use fr_mesh::{MeshData, MeshId};
use fr_render::{MeshInstance, Model};
use fr_scene::AssetPath;

/// A failure to import or validate CPU asset data.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be read or understood.
    Asset(AssetError),
    /// The asset data or a referenced handle is invalid.
    InvalidAsset(String),
}

impl fmt::Display for LoadError {
    /// Writes the failure with its cause.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(error) => error.fmt(f),
            Self::InvalidAsset(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for LoadError {
    /// Returns the import failure, when one exists.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            Self::InvalidAsset(_) => None,
        }
    }
}

impl From<AssetError> for LoadError {
    /// Wraps an import failure.
    fn from(error: AssetError) -> Self {
        Self::Asset(error)
    }
}

/// A CPU asset with a revision used to synchronize its GPU representation.
pub(crate) struct AssetEntry<T> {
    /// The current asset data.
    pub(crate) data: T,
    /// The revision incremented whenever the data changes.
    pub(crate) revision: u64,
}

/// The CPU image and its sampling and colour-space settings.
pub(crate) struct TextureAsset {
    /// The decoded pixels.
    pub(crate) image: ImageData,
    /// The sampling settings.
    pub(crate) sampler: SamplerData,
    /// Whether the image is sRGB colour.
    pub(crate) srgb: bool,
}

/// A loaded source and the assets it owns.
struct LoadedModel {
    /// The model's placements and lights.
    model: Model,
    /// Its independently owned mesh handles.
    meshes: Vec<MeshId>,
    /// Its independently owned material handles.
    materials: Vec<MaterialId>,
    /// Its independently owned texture handles.
    textures: Vec<TextureId>,
}

/// CPU asset storage with stable handles that are never reused after removal.
///
/// Systems can load, replace or unload assets without a window or graphics device.
/// The host synchronizes these assets to the renderer before drawing.
#[derive(Resource)]
pub struct Assets {
    /// Mesh data indexed by logical handle.
    pub(crate) meshes: BTreeMap<MeshId, AssetEntry<MeshData>>,
    /// Material data indexed by logical handle.
    pub(crate) materials: BTreeMap<MaterialId, AssetEntry<MaterialData<TextureId>>>,
    /// Texture data indexed by logical handle.
    pub(crate) textures: BTreeMap<TextureId, AssetEntry<TextureAsset>>,
    /// Loaded sources, shared by their source path.
    models: BTreeMap<AssetPath, LoadedModel>,
    /// The next unused mesh identifier.
    next_mesh: u32,
    /// The next unused material identifier.
    next_material: u32,
    /// The next unused texture identifier.
    next_texture: u32,
}

impl Default for Assets {
    /// Creates empty storage with the default material at handle zero.
    fn default() -> Self {
        Self {
            meshes: BTreeMap::new(),
            materials: BTreeMap::from([(
                MaterialId::from_index(0),
                AssetEntry {
                    data: MaterialData::default(),
                    revision: 0,
                },
            )]),
            textures: BTreeMap::new(),
            models: BTreeMap::new(),
            next_mesh: 0,
            next_material: 1,
            next_texture: 0,
        }
    }
}

impl Assets {
    /// Loads a source once and returns its logical mesh and material placements.
    ///
    /// # Errors
    ///
    /// Returns an error when the source cannot be imported or its data is invalid.
    pub fn load_gltf(&mut self, path: impl AsRef<Path>) -> Result<Model, LoadError> {
        let path = path.as_ref();
        let key = AssetPath(path.to_string_lossy().into_owned());
        if let Some(model) = self.models.get(&key) {
            return Ok(model.model.clone());
        }
        let data = fr_assets::load_gltf(path)?;
        let starts = (self.next_mesh, self.next_material, self.next_texture);
        let model = match self.add_model_data(&data) {
            Ok(model) => model,
            Err(error) => {
                self.remove_model_assets(starts);
                return Err(error);
            }
        };
        self.models.insert(
            key,
            LoadedModel {
                meshes: (starts.0..self.next_mesh).map(MeshId::from_index).collect(),
                materials: (starts.1..self.next_material)
                    .map(MaterialId::from_index)
                    .collect(),
                textures: (starts.2..self.next_texture)
                    .map(TextureId::from_index)
                    .collect(),
                model: model.clone(),
            },
        );
        Ok(model)
    }

    /// Returns the loaded model at a source path.
    pub fn model(&self, path: &AssetPath) -> Option<&Model> {
        self.models.get(path).map(|model| &model.model)
    }

    /// Unloads a model and its assets; existing copies of its handles become stale.
    ///
    /// # Errors
    ///
    /// Returns an error if another material references a texture owned by the model.
    pub fn unload_model(&mut self, path: &AssetPath) -> Result<bool, LoadError> {
        let Some(model) = self.models.get(path) else {
            return Ok(false);
        };
        for (id, material) in &self.materials {
            if !model.materials.contains(id)
                && material_textures(&material.data)
                    .any(|texture| model.textures.contains(&texture))
            {
                return Err(LoadError::InvalidAsset(String::from(
                    "a model texture is used by another material",
                )));
            }
        }
        if let Some(model) = self.models.remove(path) {
            for id in model.meshes {
                self.meshes.remove(&id);
            }
            for id in model.materials {
                self.materials.remove(&id);
            }
            for id in model.textures {
                self.textures.remove(&id);
            }
        }
        Ok(true)
    }

    /// Removes partial model assets after an import validation failure.
    fn remove_model_assets(&mut self, starts: (u32, u32, u32)) {
        self.meshes.retain(|id, _| id.index() < starts.0 as usize);
        self.materials
            .retain(|id, _| id.index() < starts.1 as usize);
        self.textures.retain(|id, _| id.index() < starts.2 as usize);
    }

    /// Stores a validated mesh for upload on the next frame.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent mesh data or exhausted handles.
    pub fn add_mesh(&mut self, mesh: &MeshData) -> Result<MeshId, LoadError> {
        validate_mesh(mesh)?;
        let id = MeshId::from_index(take_index(&mut self.next_mesh)?);
        self.meshes.insert(
            id,
            AssetEntry {
                data: mesh.clone(),
                revision: 0,
            },
        );
        Ok(id)
    }

    /// Returns the CPU mesh behind a logical handle.
    pub fn mesh(&self, id: MeshId) -> Option<&MeshData> {
        self.meshes.get(&id).map(|entry| &entry.data)
    }

    /// Returns the CPU material behind a logical handle.
    pub fn material(&self, id: MaterialId) -> Option<&MaterialData<TextureId>> {
        self.materials.get(&id).map(|entry| &entry.data)
    }

    /// Returns the decoded image behind a logical texture handle.
    pub fn image(&self, id: TextureId) -> Option<&ImageData> {
        self.textures.get(&id).map(|entry| &entry.data.image)
    }

    /// Replaces mesh data while retaining its handle.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data or an unknown handle.
    pub fn replace_mesh(&mut self, id: MeshId, mesh: &MeshData) -> Result<(), LoadError> {
        validate_mesh(mesh)?;
        replace_entry(&mut self.meshes, id, mesh.clone())
    }

    /// Releases a mesh; its handle is never reused.
    pub fn remove_mesh(&mut self, id: MeshId) -> bool {
        self.meshes.remove(&id).is_some()
    }

    /// Stores an image and its sampling settings for upload on the next frame.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed pixels or exhausted handles.
    pub fn add_texture(
        &mut self,
        image: &ImageData,
        sampler: SamplerData,
        srgb: bool,
    ) -> Result<TextureId, LoadError> {
        validate_image(image)?;
        let id = TextureId::from_index(take_index(&mut self.next_texture)?);
        self.textures.insert(
            id,
            AssetEntry {
                data: TextureAsset {
                    image: image.clone(),
                    sampler,
                    srgb,
                },
                revision: 0,
            },
        );
        Ok(id)
    }

    /// Replaces a texture while retaining its handle and updating dependent materials.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed pixels or an unknown handle.
    pub fn replace_texture(
        &mut self,
        id: TextureId,
        image: &ImageData,
        sampler: SamplerData,
        srgb: bool,
    ) -> Result<(), LoadError> {
        validate_image(image)?;
        replace_entry(
            &mut self.textures,
            id,
            TextureAsset {
                image: image.clone(),
                sampler,
                srgb,
            },
        )
    }

    /// Releases an unused texture; its handle is never reused.
    ///
    /// # Errors
    ///
    /// Returns an error if a material still references the texture.
    pub fn remove_texture(&mut self, id: TextureId) -> Result<bool, LoadError> {
        if self
            .materials
            .values()
            .any(|entry| material_textures(&entry.data).any(|texture| texture == id))
        {
            return Err(LoadError::InvalidAsset(String::from(
                "the texture is still used by a material",
            )));
        }
        Ok(self.textures.remove(&id).is_some())
    }

    /// Stores a material referencing logical texture handles.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown textures or exhausted handles.
    pub fn add_material(
        &mut self,
        material: &MaterialData<TextureId>,
    ) -> Result<MaterialId, LoadError> {
        self.validate_material(material)?;
        let id = MaterialId::from_index(take_index(&mut self.next_material)?);
        self.materials.insert(
            id,
            AssetEntry {
                data: material.clone(),
                revision: 0,
            },
        );
        Ok(id)
    }

    /// Replaces a material while retaining its handle.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown textures or an unknown handle.
    pub fn replace_material(
        &mut self,
        id: MaterialId,
        material: &MaterialData<TextureId>,
    ) -> Result<(), LoadError> {
        self.validate_material(material)?;
        replace_entry(&mut self.materials, id, material.clone())
    }

    /// Releases a material unless it is the default material.
    pub fn remove_material(&mut self, id: MaterialId) -> bool {
        id != self.default_material() && self.materials.remove(&id).is_some()
    }

    /// Returns the fallback material's stable logical handle.
    pub fn default_material(&self) -> MaterialId {
        MaterialId::from_index(0)
    }

    /// Validates every texture reference in a material.
    fn validate_material(&self, material: &MaterialData<TextureId>) -> Result<(), LoadError> {
        if material_textures(material).any(|id| !self.textures.contains_key(&id)) {
            return Err(LoadError::InvalidAsset(String::from(
                "a material uses an unknown texture",
            )));
        }
        Ok(())
    }
    /// Stores every texture, material and mesh of `model` and returns the placed meshes as
    /// a [`Model`].
    ///
    /// A texture used both as colour and as data is stored once for each.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError::InvalidAsset`] when the model refers to an image, texture,
    /// material or mesh it does not contain, or when one of them is invalid.
    fn add_model_data(&mut self, model: &ModelData) -> Result<Model, LoadError> {
        let mut stored: HashMap<(usize, bool), TextureId> = HashMap::new();
        let mut materials = Vec::with_capacity(model.materials.len());
        for material in &model.materials {
            let resolved = material.try_map_textures(|&texture, slot| {
                self.add_model_texture(model, texture, slot, &mut stored)
            })?;
            materials.push(self.add_material(&resolved)?);
        }
        let meshes = model
            .meshes
            .iter()
            .map(|mesh| self.add_mesh(mesh))
            .collect::<Result<Vec<_>, _>>()?;
        let mut instances = Vec::with_capacity(model.parts.len());
        for part in &model.parts {
            instances.push(MeshInstance {
                mesh: pick_item(&meshes, part.mesh)?,
                material: match part.material {
                    Some(index) => pick_item(&materials, index)?,
                    None => self.default_material(),
                },
                transform: part.transform,
            });
        }
        Ok(Model {
            instances,
            lights: model.lights.clone(),
        })
    }

    /// The texture `index` of `model` stored for `slot`, reusing an earlier upload.
    fn add_model_texture(
        &mut self,
        model: &ModelData,
        index: usize,
        slot: TextureSlot,
        stored: &mut HashMap<(usize, bool), TextureId>,
    ) -> Result<TextureId, LoadError> {
        let key = (index, slot.is_srgb());
        if let Some(&id) = stored.get(&key) {
            return Ok(id);
        }
        let missing = || LoadError::InvalidAsset(String::from("a material uses a missing texture"));
        let texture = model.textures.get(index).ok_or_else(missing)?;
        let image = model.images.get(texture.image).ok_or_else(missing)?;
        let id = self.add_texture(image, texture.sampler, slot.is_srgb())?;
        stored.insert(key, id);
        Ok(id)
    }
}

/// The item at `index` of a table of stored handles.
fn pick_item<T: Copy>(table: &[T], index: usize) -> Result<T, LoadError> {
    table.get(index).copied().ok_or_else(|| {
        LoadError::InvalidAsset(format!("a part refers to the missing item {index}"))
    })
}

/// Allocates a monotonically increasing handle index.
fn take_index(next: &mut u32) -> Result<u32, LoadError> {
    let index = *next;
    *next = next
        .checked_add(1)
        .ok_or_else(|| LoadError::InvalidAsset(String::from("asset handles exhausted")))?;
    Ok(index)
}

/// Replaces a stored value and marks its GPU representation dirty.
fn replace_entry<K: Ord, T>(
    entries: &mut BTreeMap<K, AssetEntry<T>>,
    id: K,
    data: T,
) -> Result<(), LoadError> {
    let entry = entries
        .get_mut(&id)
        .ok_or_else(|| LoadError::InvalidAsset(String::from("unknown asset handle")))?;
    let revision = entry
        .revision
        .checked_add(1)
        .ok_or_else(|| LoadError::InvalidAsset(String::from("asset revisions exhausted")))?;
    *entry = AssetEntry { data, revision };
    Ok(())
}

/// Validates the geometry required by the renderer.
fn validate_mesh(mesh: &MeshData) -> Result<(), LoadError> {
    if mesh.positions.is_empty() || mesh.indices.is_empty() || !mesh.is_consistent() {
        return Err(LoadError::InvalidAsset(String::from(
            "a mesh is empty or inconsistent",
        )));
    }
    Ok(())
}

/// Validates image dimensions and decoded pixel length.
fn validate_image(image: &ImageData) -> Result<(), LoadError> {
    let expected = (image.width as usize)
        .checked_mul(image.height as usize)
        .and_then(|size| size.checked_mul(4));
    if image.width == 0 || image.height == 0 || expected != Some(image.pixels.len()) {
        return Err(LoadError::InvalidAsset(String::from(
            "an image has invalid dimensions or pixels",
        )));
    }
    Ok(())
}

/// Iterates the texture handles referenced by a material.
fn material_textures(material: &MaterialData<TextureId>) -> impl Iterator<Item = TextureId> {
    [
        material.base_color_texture,
        material.metallic_roughness_texture,
        material.normal_texture,
        material.occlusion_texture,
        material.emissive_texture,
    ]
    .into_iter()
    .flatten()
}
