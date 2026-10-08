//! Serializable game objects, component reflection and transform hierarchies.

mod components;
mod document;
mod hierarchy;
mod plugin;

pub use components::{
    AssetPath, GlobalTransform, LocalTransform, ModelRef, Name, Parent, SceneCamera, SceneId,
    SceneLight,
};
pub use document::{SceneDocument, SceneError, SceneObject};
pub use hierarchy::{HierarchyError, HierarchyStatus, propagate_transforms};
pub use plugin::{ScenePlugin, SceneSystems};
