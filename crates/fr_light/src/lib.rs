//! Punctual light sources of the forge engine: directional, point and spot.
//!
//! The crate root is the facade and nothing else.

mod light;

pub use light::{DirectionalLight, Light, PointLight, SpotLight};
