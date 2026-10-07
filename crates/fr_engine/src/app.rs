//! The application a game implements and the context its callbacks receive.

use fr_assets::AssetLibrary;
use fr_input::InputState;
use fr_ui::{Div, Theme, div};

use crate::Scene;
use crate::error::EngineResult;
use crate::runtime::Runtime;
use crate::timing::{FixedFrameInfo, FrameInfo};

/// The scene, runtime, assets and input handed to application callbacks.
pub struct AppContext<'a> {
    /// The scene.
    pub scene: &'a mut Scene,
    /// The scene runtime: behaviours, spawns and scene transitions.
    pub runtime: &'a mut Runtime,
    /// The assets of the project.
    pub assets: &'a mut AssetLibrary,
    /// The input state of the frame.
    pub input: &'a InputState,
}

/// A game the engine loop drives.
///
/// A failing callback ends the loop, and the error is reported as the reason the
/// run failed. The engine asks for a UI tree every frame with [`App::view`] and
/// hands back the messages that tree's controls send with [`App::message`]; the
/// tree never mutates the game.
pub trait App {
    /// What the controls in the UI tree send when the user activates them.
    type Message: Clone + 'static;

    /// Called once after the startup scene is loaded, before the first frame.
    ///
    /// # Errors
    ///
    /// The reason the game cannot start.
    fn start(&mut self, _context: &mut AppContext<'_>) -> EngineResult {
        Ok(())
    }

    /// The length of a fixed step in seconds. Zero or less turns fixed steps
    /// off.
    fn fixed_timestep(&self) -> f32 {
        1.0 / 60.0
    }

    /// Called once per fixed step, before the behaviours' fixed update and the
    /// physics step.
    ///
    /// # Errors
    ///
    /// The reason the step failed.
    fn fixed_update(
        &mut self,
        _context: &mut AppContext<'_>,
        _frame: &FixedFrameInfo,
    ) -> EngineResult {
        Ok(())
    }

    /// Called once per rendered frame, after the fixed steps that frame owes
    /// and before the behaviours' update.
    ///
    /// # Errors
    ///
    /// The reason the frame failed.
    fn update(&mut self, _context: &mut AppContext<'_>, _frame: &FrameInfo) -> EngineResult {
        Ok(())
    }

    /// Applies one message sent by a control in the tree last built.
    ///
    /// # Errors
    ///
    /// The reason the message could not be applied.
    fn message(&mut self, _context: &mut AppContext<'_>, _message: Self::Message) -> EngineResult {
        Ok(())
    }

    /// Builds this frame's UI tree from the game's state, in `theme`.
    fn view(&self, _theme: &Theme) -> Div<Self::Message> {
        div()
    }

    /// Whether the engine runs fixed steps at all: the application's and the
    /// behaviours' fixed updates and the physics. A game that pauses returns
    /// false, which also leaves the bodies drawn where the last step put them.
    fn simulates_physics(&self) -> bool {
        true
    }

    /// Whether presenting waits for the display's refresh. Asked every frame.
    fn vsync(&self) -> bool {
        false
    }

    /// Checked at the end of every frame; returning true leaves the frame loop
    /// the way closing the window does.
    fn wants_exit(&self) -> bool {
        false
    }
}
