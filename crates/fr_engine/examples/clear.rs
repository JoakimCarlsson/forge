//! Opens a window and clears it to a colour that drifts over time.

use fr_engine::ui::{Rgba, Theme};
use fr_engine::{App, Frame};

/// Drifts the clear colour with elapsed time.
struct Clear {
    /// The seconds elapsed since the first frame.
    elapsed: f32,
}

impl App for Clear {
    type Message = ();

    /// Advances the elapsed time.
    fn update(&mut self, frame: &Frame) {
        self.elapsed += frame.delta_seconds;
    }

    /// A blue that pulses once every couple of seconds.
    fn clear_color(&self, _theme: &Theme) -> Rgba {
        let pulse = self.elapsed.sin().mul_add(0.5, 0.5);
        Rgba::new(0.02, 0.04, 0.05 + 0.2 * pulse, 1.0)
    }
}

fn main() -> std::process::ExitCode {
    match fr_engine::run("forge clear", Clear { elapsed: 0.0 }) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("forge: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
