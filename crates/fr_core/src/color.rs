//! Conversions between the sRGB transfer curve and linear light.

/// The linear-light value of one sRGB-encoded channel in `0..=1`.
pub fn srgb_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}
