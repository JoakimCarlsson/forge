//! The document realized into a live stage for the viewport, never simulated.
//!
//! [`Preview`] turns an authoring `Document` into an `fr_scene::Stage` through
//! the engine's registered adapters, keeps the map between authored node
//! identities and entities, and the world transforms and bounds picking reads,
//! and describes the frame seen through the editor camera. Component specific
//! knowledge comes from the `subsystems` contributions; nothing here switches on
//! a component type except the check that a node holds a simulated body.
//!
//! Modules:
//!
//! - `live`: the [`Preview`] itself
//! - `build`: realizing a document into a fresh stage
//! - `fast_path`: recognising an edit that only moved nodes
//! - `bounds`: [`NodeBounds`] and [`OrientedBox`]
//! - `frame`: the default headlight

mod bounds;
mod build;
mod fast_path;
mod frame;
mod live;

pub use bounds::{NodeBounds, OrientedBox};
pub use live::Preview;
