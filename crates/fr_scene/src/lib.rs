//! The scene of the forge engine: the entity hierarchy over component records,
//! prefab instancing, transform resolution, the structural boundary and the
//! [`Stage`], the live scene that authored components are realized into
//! through registered adapters.
//!
//! An entity owns its name, local transform, parent and ordered children, and
//! the records of its components. Handles are generational, so a handle to a
//! destroyed entity never resolves again. Removal during play goes through
//! [`Stage::queue_destroy_entity`] and [`Stage::flush_destroy_queue`], the one
//! place queued destruction happens; [`Stage::resolve_transforms`] and
//! [`Stage::apply_transform_interpolation`] are the last two calls before the
//! frame is described with [`Stage::build_render_scene`].
//!
//! Nothing here needs a window or a graphics device: the frame reaches the
//! graphics side through [`RenderResources`], which [`GpuResources`] answers over a renderer.
//!
//! Modules:
//!
//! - `hierarchy`: [`EntityHandle`], [`EntityConfig`] and the [`Hierarchy`]
//! - `stage`, `physics`, `frame`: the [`Stage`], its physics step and the frame
//!   it describes
//! - `realize`: [`Realization`] and the walk that realizes documents
//! - `adapters`: the [`ComponentAdapter`] of each built-in component type
//! - `resources`: [`RenderResources`], [`Primitive`] and [`MaterialSpec`]
//! - `gpu`: [`GpuResources`] and its [`ResourceCache`], the `RenderResources`
//!   over a `fr_render::Renderer`
//! - `props`, `error`: typed property reads and [`SceneError`]

mod adapters;
mod error;
mod frame;
mod gpu;
mod hierarchy;
mod physics;
mod props;
mod realize;
mod resources;
mod stage;

pub use adapters::{
    BodyState, CameraState, ComponentAdapter, LightKind, LightState, RagdollState, RealizeContext,
    RendererSource, RendererState, builtin_adapters,
};
pub use error::SceneError;
pub use gpu::{GpuResources, ResourceCache};
pub use hierarchy::{EntityConfig, EntityHandle, EntityTag, Hierarchy};
pub use props::Props;
pub use realize::{Realization, RealizedBehaviour, RealizedEntity, RootSpec};
pub use resources::{MaterialSpec, Primitive, RenderResources};
pub use stage::Stage;
