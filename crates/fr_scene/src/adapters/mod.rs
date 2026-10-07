//! The adapters that realize authored components into a live stage.
//!
//! An adapter owns one component type key: it turns a record into the stage's
//! live state for that type, and drops that state when the entity is destroyed.
//! Shared realization, hierarchy and frame code carries no per-type switches;
//! a new built-in type is one adapter listed in [`builtin_adapters`].
//!
//! Modules:
//!
//! - `camera`: [`CameraAdapter`] and its state
//! - `light`: [`LightAdapter`] and its state
//! - `mesh_renderer`: [`MeshRendererAdapter`] and its state
//! - `body`: [`RigidBodyAdapter`] and [`ColliderAdapter`], and the body state
//! - `ragdoll`: [`RagdollAdapter`] and its state

mod body;
mod camera;
mod light;
mod mesh_renderer;
mod ragdoll;

use std::rc::Rc;

use fr_assets::AssetLibrary;
use fr_document::{ComponentRecord, ComponentSchema};

use crate::error::SceneError;
use crate::hierarchy::EntityHandle;
use crate::props::Props;
use crate::stage::Stage;

pub use body::{BodyState, ColliderAdapter, RigidBodyAdapter};
pub use camera::{CameraAdapter, CameraState};
pub use light::{LightAdapter, LightKind, LightState};
pub use mesh_renderer::{MeshRendererAdapter, RendererSource, RendererState};
pub use ragdoll::{RagdollAdapter, RagdollState};

/// What an adapter works on while a document is realized.
pub struct RealizeContext<'a> {
    /// The stage the component is realized into.
    pub stage: &'a mut Stage,
    /// The assets a component may refer to.
    pub assets: &'a mut AssetLibrary,
    /// Messages about components that were realized inert or approximately.
    pub diagnostics: &'a mut Vec<String>,
}

/// The realization of one component type.
pub trait ComponentAdapter {
    /// The type key the adapter realizes.
    fn key(&self) -> &'static str;

    /// Creates the live state of a component on an entity. Called in adapter
    /// order, after the entity exists and its transform is resolved.
    ///
    /// # Errors
    ///
    /// The reason the component cannot be realized, which rolls the whole
    /// realization back.
    fn realize(
        &self,
        context: &mut RealizeContext<'_>,
        entity: EntityHandle,
        record: &ComponentRecord,
    ) -> Result<(), SceneError>;

    /// Drops the live state of the type on an entity that is being destroyed.
    fn release(&self, stage: &mut Stage, entity: EntityHandle);
}

/// A type's schema read from the stage's built-in defaults, to read a record
/// through.
///
/// # Errors
///
/// When the stage knows no such type, which a registered adapter never hits.
pub(crate) fn props_of<'a>(
    stage: &'a Stage,
    record: &'a ComponentRecord,
) -> Result<Props<'a>, SceneError> {
    let schema: &ComponentSchema = stage
        .defaults
        .find(&record.kind)
        .ok_or_else(|| SceneError::realization(format!("{} has no schema", record.kind)))?;
    Ok(Props::new(record, schema))
}

/// The adapters of every built-in component type, in the order an entity's
/// components are realized: a body before its colliders.
pub fn builtin_adapters() -> Vec<Rc<dyn ComponentAdapter>> {
    vec![
        Rc::new(CameraAdapter),
        Rc::new(LightAdapter),
        Rc::new(MeshRendererAdapter),
        Rc::new(RigidBodyAdapter),
        Rc::new(ColliderAdapter),
        Rc::new(RagdollAdapter),
    ]
}
