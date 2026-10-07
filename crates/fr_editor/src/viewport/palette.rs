//! The colours of everything the viewport draws over the scene.

use fr_color::Rgba;
use fr_ui::Theme;

/// The colours of the grid, the selection and the transform handles. The axes
/// come from the theme's axis tokens so the viewport and the inspector tag an
/// axis the same way; the rest are fixed here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportPalette {
    /// The X axis.
    pub axis_x: Rgba,
    /// The Y axis.
    pub axis_y: Rgba,
    /// The Z axis.
    pub axis_z: Rgba,
    /// A handle under the pointer or being dragged.
    pub hover: Rgba,
    /// The handle at the pivot that scales uniformly, and other neutral parts.
    pub neutral: Rgba,
    /// The outline of selected nodes other than the primary one.
    pub selection: Rgba,
    /// The outline of the primary selected node.
    pub primary: Rgba,
    /// The grid lines every unit.
    pub grid_minor: Rgba,
    /// The grid lines every tenth unit.
    pub grid_major: Rgba,
    /// The wire shapes components draw, unless they choose their own colour.
    pub gizmo: Rgba,
}

impl ViewportPalette {
    /// The palette of a theme.
    pub fn from_theme(theme: &Theme) -> Self {
        let colors = &theme.colors;
        Self {
            axis_x: colors.axis_x,
            axis_y: colors.axis_y,
            axis_z: colors.axis_z,
            hover: Rgba::hex(0xffe066),
            neutral: Rgba::hex(0xe6e6e6),
            selection: colors.accent.alpha(0.85),
            primary: colors.accent,
            grid_minor: Rgba::hexa(0x10181f38),
            grid_major: Rgba::hexa(0x10181f66),
            gizmo: Rgba::hex(0xd9d9d9),
        }
    }
}

impl Default for ViewportPalette {
    /// The palette of the one theme.
    fn default() -> Self {
        Self::from_theme(&Theme::default())
    }
}
