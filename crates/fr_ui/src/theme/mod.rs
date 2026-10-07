//! Design tokens: the named colours, type scale, sizes and radii every element
//! reads.
//!
//! There is exactly one look: [`Theme::default`]. A window owns it and hands it
//! to the layout and paint passes. An element names the token it wants — the
//! body step of the scale, the height of a control, the wash a hover is drawn
//! at — and resolves it against the theme in hand, never against a value
//! baked in when the element was built.

mod colors;
mod emphasis;
mod radii;
mod sizes;
mod text;

pub use colors::Colors;
pub use emphasis::Emphasis;
pub use radii::Radii;
pub use sizes::Sizes;
pub use text::{Font, TextScale, TextSize};

use fr_color::Rgba;

/// The tokens a frame is drawn from.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// The semantic colours.
    pub colors: Colors,
    /// The type scale.
    pub text: TextScale,
    /// The heights controls and bars are drawn at.
    pub size: Sizes,
    /// The corner radii.
    pub radius: Radii,
    /// The alphas the translucent parts of the window are drawn at.
    pub emphasis: Emphasis,
}

impl Default for Theme {
    /// The one look: translucent deep blue-slate panels, soft light text and a
    /// mint accent.
    fn default() -> Self {
        Self {
            colors: Colors {
                background: Rgba::hex(0x0b0f12),
                surface: Rgba::hexa(0x151e27ee),
                surface_hover: Rgba::hexa(0x263540ff),
                surface_active: Rgba::hexa(0x2f4048ff),
                surface_selected: Rgba::hexa(0x2f4146ff),
                surface_input: Rgba::hexa(0x1c2832ff),
                border: Rgba::hex(0x2a3540),
                border_variant: Rgba::hex(0x232d36),
                border_focused: Rgba::hex(0x5f7f78),
                border_selected: Rgba::hex(0x3d4b55),
                drop_target: Rgba::hexa(0x93979b80),
                text: Rgba::hex(0xd5dde3),
                text_muted: Rgba::hex(0x8e9aa2),
                text_subtle: Rgba::hex(0x66737b),
                text_on_accent: Rgba::hex(0x0e1a15),
                cursor: Rgba::hex(0xd5dde3),
                selection: Rgba::hex(0x7fd6a4),
                link: Rgba::hex(0x7fd6a4),
                accent: Rgba::hex(0x7fd6a4),
                accent_hover: Rgba::hex(0x95e0b4),
                accent_active: Rgba::hex(0x6bc392),
                success: Rgba::hex(0x7fd6a4),
                warning: Rgba::hex(0xd9a343),
                danger: Rgba::hex(0xe06c62),
                axis_x: Rgba::hex(0xd8625a),
                axis_y: Rgba::hex(0x8fc866),
                axis_z: Rgba::hex(0x5b9ae0),
            },
            text: TextScale::DEFAULT,
            size: Sizes::DEFAULT,
            radius: Radii::DEFAULT,
            emphasis: Emphasis::DEFAULT,
        }
    }
}
