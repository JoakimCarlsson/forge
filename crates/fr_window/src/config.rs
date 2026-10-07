//! Window creation parameters.

/// How the window is created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowConfig {
    /// The title shown by the window manager.
    pub title: String,
    /// The initial drawable width in logical pixels.
    pub width: u32,
    /// The initial drawable height in logical pixels.
    pub height: u32,
}

impl Default for WindowConfig {
    /// A 1280 by 720 window titled "forge".
    fn default() -> Self {
        Self {
            title: String::from("forge"),
            width: 1280,
            height: 720,
        }
    }
}
