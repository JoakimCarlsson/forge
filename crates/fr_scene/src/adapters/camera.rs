//! The camera adapter.

use fr_camera::{OrthographicProjection, PerspectiveProjection, Projection};
use fr_document::ComponentRecord;
use fr_document::builtin_schema::CAMERA;

use super::{ComponentAdapter, RealizeContext, props_of};
use crate::error::SceneError;
use crate::hierarchy::EntityHandle;
use crate::stage::Stage;

/// The live state of a camera component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraState {
    /// How the camera projects.
    pub projection: Projection,
    /// The multiplier applied to scene radiance before tonemapping.
    pub exposure: f32,
}

/// Realizes `forge.camera`: the camera looks along its entity's forward axis,
/// negative Z, with the entity's Y as up.
pub struct CameraAdapter;

impl ComponentAdapter for CameraAdapter {
    /// The camera type key.
    fn key(&self) -> &'static str {
        CAMERA
    }

    /// Stores the projection, and makes the camera current when it asks to be
    /// and the scene has none yet.
    fn realize(
        &self,
        context: &mut RealizeContext<'_>,
        entity: EntityHandle,
        record: &ComponentRecord,
    ) -> Result<(), SceneError> {
        let props = props_of(context.stage, record)?;
        let near = props.float("near");
        let far = props.float("far");
        let projection = if props.text("projection") == "orthographic" {
            Projection::Orthographic(OrthographicProjection {
                height: props.float("orthographic_height"),
                near,
                far,
            })
        } else {
            Projection::Perspective(PerspectiveProjection {
                fov_y: props.float("fov_y_degrees").to_radians(),
                near,
                far,
            })
        };
        let state = CameraState {
            projection,
            exposure: props.float("exposure"),
        };
        let current = props.boolean("current");
        let stage = &mut *context.stage;
        stage.cameras.insert(entity, state);
        if current && !stage.hierarchy.contains(stage.current_camera) {
            stage.current_camera = entity;
        }
        Ok(())
    }

    /// Drops the camera, and the current camera with it.
    fn release(&self, stage: &mut Stage, entity: EntityHandle) {
        stage.cameras.remove(&entity);
        if stage.current_camera == entity {
            stage.current_camera = EntityHandle::NULL;
        }
    }
}
