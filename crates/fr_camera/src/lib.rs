//! Cameras of the forge engine: where the viewer is, how it projects, what it sees.
//!
//! A [`Camera`] looks from a position at a target through a [`Projection`]; it
//! gives the matrices a renderer draws with, the [`Frustum`] it sees, and the
//! conversions between world space and a [`Viewport`], including the
//! [`fr_math::Ray3d`] under a pointer. The [`OrbitController`] drives a camera
//! from drags and scrolls and the [`FlyController`] flies one with mouse look and movement keys.
//!
//! The crate root is the facade and nothing else.

mod camera;
mod controller;
mod fly;
mod frustum;
mod projection;
mod viewport;

pub use camera::Camera;
pub use controller::OrbitController;
pub use fly::FlyController;
pub use frustum::Frustum;
pub use projection::{OrthographicProjection, PerspectiveProjection, Projection};
pub use viewport::Viewport;
