//! The entry point of a game: opening the project, loading the startup scene
//! and driving the frame loop.

use std::process::ExitCode;

use fr_window::WindowConfig;

use crate::app::App;
use crate::error::{EngineError, EngineResult};
use crate::host::Host;
use crate::options::RunOptions;
use crate::registry::ComponentRegistry;
use crate::simulation::Simulation;

/// Opens the project, loads the startup scene and starts the application, all
/// before a window exists, so a failure here never shows one.
fn prepare<A: App>(
    app: &mut A,
    registry: ComponentRegistry,
    options: &RunOptions,
) -> EngineResult<Simulation> {
    let mut simulation = Simulation::open(registry, options)?;
    for problem in simulation.assets.index().problems() {
        eprintln!("forge: {problem}");
    }
    if let Some(scene) = simulation.startup_scene(options) {
        simulation.load_scene(&scene)?;
        for message in simulation.runtime.diagnostics() {
            eprintln!("forge: {message}");
        }
    }
    simulation.start(app)?;
    Ok(simulation)
}

/// Opens the window and drives the frame loop until it ends, then tears the
/// behaviours down.
fn drive<A: App>(
    app: &mut A,
    simulation: Simulation,
    options: &RunOptions,
) -> Result<(), EngineError> {
    let mut host = Host::new(app, simulation, options.max_frames);
    let config: WindowConfig = options.window.clone();
    let ran = fr_window::run(config, &mut host);
    let torn_down = host.simulation.shutdown();
    ran?;
    if let Some(error) = host.failure.take() {
        return Err(error);
    }
    torn_down
}

/// Runs a game: opens the project and loads its startup scene, calls
/// `App::start`, opens a window and drives the frame loop until the window
/// closes.
///
/// Each fixed step runs the application's `fixed_update`, then the behaviours'
/// fixed updates, then the physics step. Each frame then runs `App::update`,
/// the behaviours' update, the user interface, the structural boundary where
/// destruction, spawning and scene replacement happen, transform resolution and
/// interpolation, and the draw.
///
/// Returns [`ExitCode::SUCCESS`] on a clean exit and [`ExitCode::FAILURE`]
/// after a fatal error, which is written to standard error as
/// `forge: <message>`. A failing `App` or `Behavior` callback ends the loop and
/// is such an error, and so is a scene that fails to load or a device that
/// cannot be created.
pub fn run<A: App>(app: &mut A, registry: ComponentRegistry, options: &RunOptions) -> ExitCode {
    let outcome =
        prepare(app, registry, options).and_then(|simulation| drive(app, simulation, options));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("forge: {error}");
            ExitCode::FAILURE
        }
    }
}
