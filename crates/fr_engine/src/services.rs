//! What the runtime needs besides itself to run: the scene, the assets and the
//! input a frame is played against.

use fr_assets::AssetLibrary;
use fr_input::InputState;

use crate::Scene;

/// The objects a runtime call works on, borrowed from whoever owns them.
pub struct Services<'a> {
    /// The scene the runtime drives.
    pub scene: &'a mut Scene,
    /// The assets of the project.
    pub assets: &'a mut AssetLibrary,
    /// The input state of the frame.
    pub input: &'a InputState,
}
