//! What the editor knows about each component type, contributed one subsystem
//! at a time.
//!
//! A [`Subsystem`] answers for one component type key with its icon, the local
//! bounds picking uses, the wire gizmo drawn when the node is selected, the
//! entries it adds to the hierarchy's Create menu and, optionally, a
//! specialised [`InspectorContribution`]. All of it is data: a gizmo is 3D line
//! segments collected in a [`GizmoSink`], a menu entry is a [`CreateEntry`] and
//! an inspector row is an [`InspectorRow`] carrying the command an edit becomes,
//! so panels and the viewport render them without knowing any component type.
//! A type with no contribution still works: it gets a fallback icon, no bounds,
//! no gizmo and the generic rows of its schema from [`generic_rows`].
//!
//! # Adding a subsystem
//!
//! 1. Add a folder under `subsystems/` with a `mod.rs` facade, a `subsystem.rs`
//!    that implements [`Subsystem`] for the type's key from
//!    `fr_document::builtin_schema` (or a game's key), and a
//!    `register(registry: &mut SubsystemRegistry)` that adds it.
//! 2. Add one line to [`register_builtin_subsystems`] in `registry.rs`, the one
//!    place that names the subsystem folders.
//!
//! Neither `viewport` nor `preview` is ever edited for a new component type.
//!
//! Modules:
//!
//! - `subsystem`, `properties`, `context`: the [`Subsystem`] trait, typed reads
//!   of a record and what bounds may consult
//! - `gizmo_sink`, `create_entry`, `inspector`: the data contributions return
//! - `registry`: the [`SubsystemRegistry`] and [`register_builtin_subsystems`]
//! - `transform`, `mesh_renderer`, `camera`, `light`, `physics`: the built-in
//!   subsystems

mod context;
mod create_entry;
mod gizmo_sink;
mod inspector;
mod properties;
mod registry;
mod subsystem;

pub mod camera;
pub mod light;
pub mod mesh_renderer;
pub mod physics;
pub mod transform;

pub use context::BoundsContext;
pub use create_entry::CreateEntry;
pub use gizmo_sink::{GizmoLine, GizmoSink, plane_basis};
pub use inspector::{
    InspectorContext, InspectorContribution, InspectorRow, RowOption, RowValue, euler_degrees,
    generic_rows, no_change, property_row, quat_from_degrees,
};
pub use properties::Properties;
pub use registry::{SubsystemRegistry, register_builtin_subsystems};
pub use subsystem::Subsystem;
