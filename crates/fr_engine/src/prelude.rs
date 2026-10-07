//! What a game imports: `use fr_engine::prelude::*;`.

pub use fr_document::{
    AssetReference, Color, ComponentSchema, EntityReference, Guid, PropertyOverride,
    PropertySchema, PropertyType, PropertyValue,
};
pub use fr_input::{InputState, PointerButton};
pub use fr_math::{Quat, Vec2, Vec3};
pub use fr_scene::{EntityConfig, EntityHandle};
pub use fr_transform::Transform;

pub use crate::{
    App, AppContext, AssetLibrary, Behavior, BehaviorContext, BehaviorId, ComponentRegistry,
    EngineError, EngineResult, FixedFrameInfo, FrameInfo, Runtime, Scene, SpawnRequest,
    SpawnResult, SpawnState, SpawnToken,
};
