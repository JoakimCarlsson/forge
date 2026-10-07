//! The crate a game depends on: the [`App`] a game implements, the [`Behavior`]s
//! it attaches to entities, the [`ComponentRegistry`] it registers them in, the
//! [`Runtime`] that loads scenes and runs the behaviours, the [`Simulation`] a
//! frame loop drives step by step and [`run`].
//!
//! The live scene is the `fr_scene` stage, named [`Scene`] here. Nothing in the
//! simulation needs a window or a graphics device. Every engine crate a game
//! needs is re-exported under a short module name. No winit or wgpu type
//! appears in this crate's public API.
//!
//! A game registers its component types in explicit code, so no behaviour
//! registers itself as a side effect of being linked:
//!
//! ```ignore
//! let mut registry = ComponentRegistry::new();
//! registry.add_behavior(spinner_schema(), || Box::new(Spinner::default()));
//! if export_schema_if_requested(&registry, &arguments)? {
//!     return Ok(());
//! }
//! ```
//!
//! Modules:
//!
//! - `app`: the [`App`] trait and its context
//! - `behavior`: the [`Behavior`] trait and the context of its callbacks
//! - `registry`: the [`ComponentRegistry`] of component types and factories
//! - `runtime`: the [`Runtime`], its spawns and its structural boundary
//! - `simulation`, `services`: the [`Simulation`] state and the objects a
//!   runtime call works on
//! - `options`, `run`: [`RunOptions`], [`parse_run_options`] and [`run`]
//! - `schema`: the export of the game schema a tool reads
//! - `host`: the frame loop's host
//! - `timing`, `error`: [`FrameInfo`], [`FixedFrameInfo`] and [`EngineError`]
//! - `prelude`: what a game imports

mod app;
mod behavior;
mod error;
mod host;
mod options;
pub mod prelude;
mod registry;
mod run;
mod runtime;
mod schema;
mod services;
mod simulation;
mod timing;

pub use app::{App, AppContext};
pub use behavior::{Behavior, BehaviorContext, BehaviorId};
pub use error::{EngineError, EngineResult};
pub use fr_assets as assets;
pub use fr_assets::AssetLibrary;
pub use fr_camera as camera;
pub use fr_color as color;
pub use fr_document as document;
pub use fr_image as image;
pub use fr_input as input;
pub use fr_light as light;
pub use fr_material as material;
pub use fr_math as math;
pub use fr_mesh as mesh;
pub use fr_physics as physics;
pub use fr_project as project;
pub use fr_render as render;
pub use fr_scene as scene;
pub use fr_scene::Stage as Scene;
pub use fr_skeleton as skeleton;
pub use fr_time as time;
pub use fr_transform as transform;
pub use fr_ui as ui;
pub use options::{RunOptions, WindowConfig, parse_run_options};
pub use registry::{BehaviorFactory, ComponentRegistry};
pub use run::run;
pub use runtime::{Runtime, SpawnRequest, SpawnResult, SpawnState, SpawnToken};
pub use schema::{
    EXPORT_SCHEMA_OPTION, export_game_schema, export_schema_if_requested, save_game_schema,
};
pub use services::Services;
pub use simulation::Simulation;
pub use timing::{FixedFrameInfo, FrameInfo};

/// An entity of the scene.
pub type EntityHandle = fr_scene::EntityHandle;
