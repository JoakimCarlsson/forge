//! What a frame needs from the graphics side, named without a device: the
//! primitives and materials a stage draws, and the trait the host answers them
//! with.

use std::sync::Arc;

use fr_document::Guid;
use fr_material::MaterialId;
use fr_mesh::MeshId;
use fr_render::Model;

use crate::error::SceneError;

/// How finely a capsule's size is quantised, in steps per world unit, so that
/// equal capsules share one mesh.
const CAPSULE_STEPS_PER_UNIT: f32 = 1000.0;

/// A mesh the engine generates, drawn where a model is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// A cube of side one.
    Cube,
    /// A sphere of diameter one.
    Sphere,
    /// A capsule of a radius and a length of straight core, in thousandths.
    Capsule {
        /// The radius in thousandths of a unit.
        radius_millis: u32,
        /// The length of the cylinder between the caps in thousandths of a unit.
        length_millis: u32,
    },
}

impl Primitive {
    /// A capsule of a radius and a core length, rounded to thousandths.
    pub fn capsule(radius: f32, length: f32) -> Self {
        let quantise = |value: f32| (value.max(0.0) * CAPSULE_STEPS_PER_UNIT).round() as u32;
        Self::Capsule {
            radius_millis: quantise(radius),
            length_millis: quantise(length),
        }
    }
}

/// A material described by its factors, without textures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialSpec {
    /// The linear base colour and opacity.
    pub base_color: [f32; 4],
    /// How metallic the surface is, in zero to one.
    pub metallic: f32,
    /// How rough the surface is, in zero to one.
    pub roughness: f32,
    /// The linear colour the surface emits, already scaled by its intensity.
    pub emissive: [f32; 3],
    /// Whether back faces are drawn.
    pub double_sided: bool,
}

impl MaterialSpec {
    /// An opaque material of a colour with no emission.
    pub fn solid(color: [f32; 3], metallic: f32, roughness: f32) -> Self {
        Self {
            base_color: [color[0], color[1], color[2], 1.0],
            metallic,
            roughness,
            emissive: [0.0; 3],
            double_sided: false,
        }
    }

    /// A key that is equal for equal materials, for caching what a host made.
    pub fn cache_key(&self) -> [u32; 10] {
        let [r, g, b, a] = self.base_color;
        let [er, eg, eb] = self.emissive;
        [
            r.to_bits(),
            g.to_bits(),
            b.to_bits(),
            a.to_bits(),
            self.metallic.to_bits(),
            self.roughness.to_bits(),
            er.to_bits(),
            eg.to_bits(),
            eb.to_bits(),
            u32::from(self.double_sided),
        ]
    }
}

/// The meshes, materials and models a host puts on the graphics device for a
/// stage's frame, created the first time they are asked for.
pub trait RenderResources {
    /// The uploaded model of an asset.
    ///
    /// # Errors
    ///
    /// When the asset cannot be loaded or uploaded.
    fn model(&mut self, id: Guid) -> Result<Arc<Model>, SceneError>;

    /// The mesh of a primitive.
    ///
    /// # Errors
    ///
    /// When the mesh cannot be uploaded.
    fn primitive(&mut self, primitive: Primitive) -> Result<MeshId, SceneError>;

    /// The material of a specification.
    ///
    /// # Errors
    ///
    /// When the material cannot be created.
    fn material(&mut self, spec: &MaterialSpec) -> Result<MaterialId, SceneError>;

    /// The material of anything without one of its own.
    fn default_material(&self) -> MaterialId;
}
