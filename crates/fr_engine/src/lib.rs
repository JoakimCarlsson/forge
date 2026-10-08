//! The ECS application facade, host and subsystem exports of the forge engine.

mod application;
mod extraction;
mod frame;
mod host;
mod loader;
mod simulation;
mod uploads;

pub use application::{App, AppFailure, Extract, Inputs, UiMessages, WindowSettings};
pub use extraction::{MeshRenderer, RenderScene, RenderSystems};
pub use fr_app as app;
pub use fr_app::{Plugin, Runtime};
pub use fr_app::{ecs, reflect};
pub use fr_assets as assets;
pub use fr_camera as camera;
pub use fr_color as color;
pub use fr_image as image;
pub use fr_input as input;
pub use fr_light as light;
pub use fr_material as material;
pub use fr_math as math;
pub use fr_mesh as mesh;
pub use fr_physics as physics;
pub use fr_render as render;
pub use fr_scene as scene;
pub use fr_time as time;
pub use fr_transform as transform;
pub use fr_ui as ui;
pub use frame::{FixedStep, Frame, Input};
pub use host::{EngineError, run};
pub use loader::{Assets, LoadError};
pub use simulation::{PhysicsPlugin, PhysicsSettings, PhysicsSystems, PhysicsWorld, RigidBody};
