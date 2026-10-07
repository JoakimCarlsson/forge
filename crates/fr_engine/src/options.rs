//! How a run is configured: the project, the startup scene and the window.

use std::ffi::OsString;
use std::path::PathBuf;

pub use fr_window::WindowConfig;

/// What `run` is asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOptions {
    /// The window, read once when it is created.
    pub window: WindowConfig,
    /// The project directory. Empty uses the working directory, which is where a
    /// game started by `make run` or by an editor finds its project.
    pub project_root: PathBuf,
    /// The scene to start in, relative to the asset root. Empty uses the
    /// project's startup scene, and a run with neither starts with an empty
    /// scene.
    pub startup_scene: PathBuf,
    /// Zero runs until the window closes; otherwise the loop ends after that
    /// many frames.
    pub max_frames: u64,
}

impl Default for RunOptions {
    /// A 1280 by 720 window titled "forge" that starts in the project's startup
    /// scene and runs until it closes.
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            project_root: PathBuf::new(),
            startup_scene: PathBuf::new(),
            max_frames: 0,
        }
    }
}

/// Fills [`RunOptions`] from the command line, the program name first:
/// `--project <dir>`, `--scene <path>` (relative to the asset root), `--frames
/// <count>`, `--width <pixels>` and `--height <pixels>`. Unknown arguments are
/// ignored, so a game is free to read its own alongside these, and a value that
/// is missing or not a number leaves the default.
pub fn parse_run_options(arguments: impl IntoIterator<Item = OsString>) -> RunOptions {
    let mut options = RunOptions::default();
    let mut arguments = arguments.into_iter().skip(1);
    while let Some(argument) = arguments.next() {
        let number = |value: Option<OsString>| {
            value.and_then(|text| text.to_string_lossy().parse::<u64>().ok())
        };
        match argument.to_string_lossy().as_ref() {
            "--project" => {
                if let Some(value) = arguments.next() {
                    options.project_root = PathBuf::from(value);
                }
            }
            "--scene" => {
                if let Some(value) = arguments.next() {
                    options.startup_scene = PathBuf::from(value);
                }
            }
            "--frames" => options.max_frames = number(arguments.next()).unwrap_or(0),
            "--width" => {
                if let Some(width) = number(arguments.next()).and_then(|v| u32::try_from(v).ok()) {
                    options.window.width = width;
                }
            }
            "--height" => {
                if let Some(height) = number(arguments.next()).and_then(|v| u32::try_from(v).ok()) {
                    options.window.height = height;
                }
            }
            _ => {}
        }
    }
    options
}
