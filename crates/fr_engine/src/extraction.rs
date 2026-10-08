//! Extraction of ECS components into the renderer's disposable frame description.

use fr_app::ecs as bevy_ecs;
use fr_app::ecs::{
    component::Component, entity::Entity, resource::Resource, schedule::SystemSet, world::World,
};
use fr_material::MaterialId;
use fr_math::Mat4;
use fr_mesh::MeshId;
use fr_render::Scene;
use fr_scene::{GlobalTransform, HierarchyStatus, ModelRef, SceneCamera, SceneLight};
use fr_transform::Transform;

use crate::{AppFailure, Assets};

/// The ordered render extraction stages.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RenderSystems {
    /// Extracts authored scene and runtime mesh components.
    Extract,
}

/// A runtime mesh placement referencing logical CPU asset handles.
#[derive(Component, Clone, Copy, Debug)]
pub struct MeshRenderer {
    /// The mesh to draw.
    pub mesh: MeshId,
    /// The material to apply.
    pub material: MaterialId,
}

/// The renderer frame produced by extraction systems.
#[derive(Resource, Default)]
pub struct RenderScene(pub Scene);

/// Loads referenced models and extracts components in entity order.
pub(crate) fn extract_scene(world: &mut World) {
    if let Err(error) = &world.resource::<HierarchyStatus>().0 {
        let reason = error.to_string();
        world.resource_mut::<AppFailure>().0 = Some(reason);
        return;
    }
    let mut query = world.query::<(
        Entity,
        Option<&GlobalTransform>,
        Option<&ModelRef>,
        Option<&MeshRenderer>,
        Option<&SceneCamera>,
        Option<&SceneLight>,
    )>();
    let mut objects: Vec<_> = query
        .iter(world)
        .map(|(entity, global, model, mesh, camera, light)| {
            (
                entity,
                global.map_or(Mat4::IDENTITY, |global| global.0),
                model.cloned(),
                mesh.copied(),
                camera.cloned(),
                light.cloned(),
            )
        })
        .collect();
    objects.sort_by_key(|object| object.0);
    let mut extracted = Scene::default();
    let mut camera_selected = false;
    for (_, matrix, model, mesh, camera, light) in objects {
        let transform = Transform::from_matrix(matrix);
        if let Some(model) = model {
            if world.resource::<Assets>().model(&model.0).is_none()
                && let Err(error) = world.resource_mut::<Assets>().load_gltf(&model.0.0)
            {
                world.resource_mut::<AppFailure>().0 = Some(error.to_string());
                return;
            }
            if let Some(model) = world.resource::<Assets>().model(&model.0) {
                extracted.add_model(model, transform);
            }
        }
        if let Some(mesh) = mesh {
            extracted.add(mesh.mesh, mesh.material, transform);
        }
        if let Some(camera) = camera
            && camera.active
            && !camera_selected
        {
            extracted.camera = camera.camera(matrix);
            camera_selected = true;
        }
        if let Some(light) = light {
            extracted.add_light(light.light(matrix));
        }
    }
    let mut frame = world.resource_mut::<RenderScene>();
    if camera_selected {
        frame.0.camera = extracted.camera;
    }
    frame.0.instances.extend(extracted.instances);
    frame.0.lights.extend(extracted.lights);
}
