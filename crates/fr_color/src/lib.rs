//! Colours of the forge engine: sRGB with straight alpha, and the transfer curve to linear light.
//!
//! The crate root is the facade and nothing else.

mod rgba;
mod srgb;

pub use rgba::Rgba;
pub use srgb::srgb_to_linear;
