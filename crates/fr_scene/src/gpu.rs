//! Putting what a frame draws on the graphics device: glTF models, primitive
//! meshes and materials, uploaded the first time a frame asks for them and
//! cached after that. [`GpuResources`] is the [`RenderResources`] a host or an
//! editor answers a frame with when it owns a [`Renderer`].

use std::collections::HashMap;
use std::sync::Arc;

use fr_assets::{AssetLibrary, ModelData};
use fr_document::Guid;
use fr_image::TextureId;
use fr_material::{AlphaMode, MaterialData, MaterialId, TextureSlot};
use fr_math::{Vec3, Vec4};
use fr_mesh::{MeshId, capsule, cube, sphere};
use fr_render::{MeshInstance, Model, RenderError, Renderer};

use crate::error::SceneError;
use crate::resources::{MaterialSpec, Primitive, RenderResources};

/// The segments around a sphere or capsule.
const ROUND_SEGMENTS: u32 = 32;

/// The rings of a sphere from pole to pole.
const SPHERE_RINGS: u32 = 16;

/// The segments around a capsule, fewer than a sphere since capsules are small.
const CAPSULE_SEGMENTS: u32 = 24;

/// The rings of each cap of a capsule.
const CAPSULE_RINGS: u32 = 8;

/// What is on the device so far.
#[derive(Default)]
pub struct ResourceCache {
    /// The uploaded models by asset identity.
    models: HashMap<Guid, Arc<Model>>,
    /// The uploaded primitive meshes.
    primitives: HashMap<Primitive, MeshId>,
    /// The created materials by the key of their specification.
    materials: HashMap<[u32; 10], MaterialId>,
}

/// The resources of one frame: a renderer to upload to, the assets to read from
/// and the cache of what was uploaded before.
pub struct GpuResources<'a> {
    /// The renderer uploads go to.
    pub renderer: &'a mut Renderer,
    /// The assets models are read from.
    pub assets: &'a mut AssetLibrary,
    /// What was uploaded in earlier frames.
    pub cache: &'a mut ResourceCache,
}

/// The mesh data of a primitive.
fn primitive_mesh(primitive: Primitive) -> fr_mesh::MeshData {
    match primitive {
        Primitive::Cube => cube(1.0),
        Primitive::Sphere => sphere(0.5, ROUND_SEGMENTS, SPHERE_RINGS),
        Primitive::Capsule {
            radius_millis,
            length_millis,
        } => capsule(
            radius_millis as f32 / 1000.0,
            length_millis as f32 / 1000.0,
            CAPSULE_SEGMENTS,
            CAPSULE_RINGS,
        ),
    }
}

/// The material data of a specification: translucent when its colour is.
fn material_data(spec: &MaterialSpec) -> MaterialData<TextureId> {
    let [r, g, b, a] = spec.base_color;
    MaterialData {
        base_color: Vec4::new(r, g, b, a),
        metallic: spec.metallic,
        roughness: spec.roughness,
        emissive: Vec3::from_array(spec.emissive),
        alpha_mode: if a < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        double_sided: spec.double_sided,
        ..MaterialData::default()
    }
}

/// The failure of an upload as a scene error.
fn upload_failed(error: &RenderError) -> SceneError {
    SceneError::resource(error.to_string())
}

impl RenderResources for GpuResources<'_> {
    /// The uploaded model of an asset, uploading it the first time.
    fn model(&mut self, id: Guid) -> Result<Arc<Model>, SceneError> {
        if let Some(model) = self.cache.models.get(&id) {
            return Ok(Arc::clone(model));
        }
        let data = self.assets.model(id)?;
        let model = Arc::new(upload_model(self.renderer, &data).map_err(|e| upload_failed(&e))?);
        self.cache.models.insert(id, Arc::clone(&model));
        Ok(model)
    }

    /// The mesh of a primitive, uploading it the first time.
    fn primitive(&mut self, primitive: Primitive) -> Result<MeshId, SceneError> {
        if let Some(mesh) = self.cache.primitives.get(&primitive) {
            return Ok(*mesh);
        }
        let mesh = self
            .renderer
            .create_mesh(&primitive_mesh(primitive))
            .map_err(|e| upload_failed(&e))?;
        self.cache.primitives.insert(primitive, mesh);
        Ok(mesh)
    }

    /// The material of a specification, creating it the first time.
    fn material(&mut self, spec: &MaterialSpec) -> Result<MaterialId, SceneError> {
        let key = spec.cache_key();
        if let Some(material) = self.cache.materials.get(&key) {
            return Ok(*material);
        }
        let material = self
            .renderer
            .create_material(&material_data(spec))
            .map_err(|e| upload_failed(&e))?;
        self.cache.materials.insert(key, material);
        Ok(material)
    }

    /// The material of anything without one of its own.
    fn default_material(&self) -> MaterialId {
        self.renderer.default_material()
    }
}

/// Uploads every texture, material and mesh of `model` and returns the placed
/// meshes as a [`Model`]. A texture used both as colour and as data is uploaded
/// once for each.
///
/// # Errors
///
/// [`RenderError::InvalidAsset`] when the model refers to an image, texture,
/// material or mesh it does not contain, or when one of them is invalid.
fn upload_model(renderer: &mut Renderer, model: &ModelData) -> Result<Model, RenderError> {
    let mut uploaded: HashMap<(usize, bool), TextureId> = HashMap::new();
    let mut materials = Vec::with_capacity(model.materials.len());
    for material in &model.materials {
        let resolved = material.try_map_textures(|&texture, slot| {
            upload_texture(renderer, model, texture, slot, &mut uploaded)
        })?;
        materials.push(renderer.create_material(&resolved)?);
    }
    let meshes = model
        .meshes
        .iter()
        .map(|mesh| renderer.create_mesh(mesh))
        .collect::<Result<Vec<_>, _>>()?;
    let mut instances = Vec::with_capacity(model.parts.len());
    for part in &model.parts {
        instances.push(MeshInstance {
            mesh: pick_item(&meshes, part.mesh)?,
            material: match part.material {
                Some(index) => pick_item(&materials, index)?,
                None => renderer.default_material(),
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
fn upload_texture(
    renderer: &mut Renderer,
    model: &ModelData,
    index: usize,
    slot: TextureSlot,
    uploaded: &mut HashMap<(usize, bool), TextureId>,
) -> Result<TextureId, RenderError> {
    let key = (index, slot.is_srgb());
    if let Some(&id) = uploaded.get(&key) {
        return Ok(id);
    }
    let missing = || RenderError::InvalidAsset(String::from("a material uses a missing texture"));
    let texture = model.textures.get(index).ok_or_else(missing)?;
    let image = model.images.get(texture.image).ok_or_else(missing)?;
    let id = renderer.create_texture(image, texture.sampler, slot.is_srgb())?;
    uploaded.insert(key, id);
    Ok(id)
}

/// The item at `index` of a table of uploaded handles.
fn pick_item<T: Copy>(table: &[T], index: usize) -> Result<T, RenderError> {
    table.get(index).copied().ok_or_else(|| {
        RenderError::InvalidAsset(format!("a part refers to the missing item {index}"))
    })
}
