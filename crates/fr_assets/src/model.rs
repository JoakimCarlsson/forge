//! A whole imported file: geometry, materials, textures and where each part sits.

use fr_image::{ImageData, TextureData};
use fr_light::Light;
use fr_material::MaterialData;
use fr_mesh::MeshData;
use fr_transform::Transform;

/// One mesh placed in the model with one material.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelPart {
    /// The name of the node that placed it, when it has one.
    pub name: Option<String>,
    /// The index of the mesh in [`ModelData::meshes`].
    pub mesh: usize,
    /// The index of the material in [`ModelData::materials`], or none for the default material.
    pub material: Option<usize>,
    /// Where the part sits in the model's space, with every parent node applied.
    pub transform: Transform,
}

/// The contents of an imported file, ready for a renderer to upload.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelData {
    /// The decoded images.
    pub images: Vec<ImageData>,
    /// The textures, each an image and a sampler.
    pub textures: Vec<TextureData>,
    /// The materials, whose texture slots index [`ModelData::textures`].
    pub materials: Vec<MaterialData<usize>>,
    /// The meshes, one per triangle primitive.
    pub meshes: Vec<MeshData>,
    /// The mesh placements, flattened from the node hierarchy.
    pub parts: Vec<ModelPart>,
    /// The lights, placed in the model's space.
    pub lights: Vec<Light>,
}
