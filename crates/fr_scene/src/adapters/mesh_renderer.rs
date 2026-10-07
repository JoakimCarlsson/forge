//! The mesh renderer adapter.

use fr_document::builtin_schema::MESH_RENDERER;
use fr_document::{ComponentRecord, Guid};

use super::{ComponentAdapter, RealizeContext, props_of};
use crate::error::SceneError;
use crate::hierarchy::EntityHandle;
use crate::resources::{MaterialSpec, Primitive};
use crate::stage::Stage;

/// What a mesh renderer draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RendererSource {
    /// The model of a glTF asset.
    Model(Guid),
    /// A mesh the engine generates.
    Primitive(Primitive),
}

/// The live state of a mesh renderer component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RendererState {
    /// What is drawn.
    pub source: RendererSource,
    /// The material that replaces the model's own, when overridden.
    pub material: Option<MaterialSpec>,
}

/// Realizes `forge.mesh_renderer`: draws a model or a primitive at its entity,
/// with its own materials or one override.
pub struct MeshRendererAdapter;

impl ComponentAdapter for MeshRendererAdapter {
    /// The mesh renderer type key.
    fn key(&self) -> &'static str {
        MESH_RENDERER
    }

    /// Stores what to draw. A model is loaded now, so a missing or unreadable
    /// asset fails the realization instead of the first frame.
    fn realize(
        &self,
        context: &mut RealizeContext<'_>,
        entity: EntityHandle,
        record: &ComponentRecord,
    ) -> Result<(), SceneError> {
        let props = props_of(context.stage, record)?;
        let model = props.asset("model");
        let source = if model.asset.valid() {
            context.assets.model(model.asset)?;
            RendererSource::Model(model.asset)
        } else {
            match props.text("primitive") {
                "cube" => RendererSource::Primitive(Primitive::Cube),
                "sphere" => RendererSource::Primitive(Primitive::Sphere),
                "capsule" => RendererSource::Primitive(Primitive::capsule(
                    props.float("radius"),
                    props.float("length"),
                )),
                _ => {
                    context
                        .diagnostics
                        .push("a mesh renderer has no model or primitive".to_owned());
                    return Ok(());
                }
            }
        };
        let material = props.boolean("override_material").then(|| {
            let color = props.color("base_color");
            let emissive = props.color("emissive");
            let intensity = props.float("emissive_intensity");
            MaterialSpec {
                base_color: [color.r, color.g, color.b, color.a],
                metallic: props.float("metallic"),
                roughness: props.float("roughness"),
                emissive: [
                    emissive.r * intensity,
                    emissive.g * intensity,
                    emissive.b * intensity,
                ],
                double_sided: props.boolean("double_sided"),
            }
        });
        context
            .stage
            .renderers
            .insert(entity, RendererState { source, material });
        Ok(())
    }

    /// Drops the renderer.
    fn release(&self, stage: &mut Stage, entity: EntityHandle) {
        stage.renderers.remove(&entity);
    }
}
