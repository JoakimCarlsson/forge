//! The state of a run and the steps of a frame, with no window and no graphics
//! device: a game or tool can drive the same steps itself.

use std::path::Path;

use fr_assets::AssetLibrary;
use fr_input::InputState;
use fr_project::Project;

use crate::Scene;
use crate::app::{App, AppContext};
use crate::error::EngineResult;
use crate::options::RunOptions;
use crate::registry::ComponentRegistry;
use crate::runtime::Runtime;
use crate::services::Services;
use crate::timing::{FixedFrameInfo, FrameInfo};

/// The scene, runtime, assets and input of a run, and one method per step of a
/// frame.
///
/// The order of a frame is written once, in the host that drives it: per fixed
/// step `fixed_update`; per rendered frame `update`, the user interface,
/// `structural_boundary`, `prepare_frame` and the draw.
pub struct Simulation {
    /// The live scene.
    pub scene: Scene,
    /// The behaviours, spawns and scene transitions.
    pub runtime: Runtime,
    /// The assets of the project.
    pub assets: AssetLibrary,
    /// The input of the frame.
    pub input: InputState,
    /// The project the run opened, when it is in one.
    project: Option<Project>,
}

impl Simulation {
    /// A simulation over an asset library with nothing loaded.
    pub fn new(registry: ComponentRegistry, assets: AssetLibrary) -> Self {
        Self {
            scene: Scene::new(),
            runtime: Runtime::new(registry),
            assets,
            input: InputState::new(),
            project: None,
        }
    }

    /// Opens the project of a run: the directory named by the options, or the
    /// working directory. A directory with a `project.forge` makes its asset
    /// root the library's root; any other directory is the asset root itself.
    ///
    /// # Errors
    ///
    /// When the working directory is unknown or `project.forge` cannot be read.
    pub fn open(registry: ComponentRegistry, options: &RunOptions) -> EngineResult<Self> {
        let root = if options.project_root.as_os_str().is_empty() {
            std::env::current_dir().map_err(|error| {
                crate::error::EngineError::new(format!("no working directory: {error}"))
            })?
        } else {
            options.project_root.clone()
        };
        if Project::exists_in(&root) {
            let project = Project::open(&root)?;
            let mut simulation = Self::new(registry, AssetLibrary::open(&project.asset_root()));
            simulation.project = Some(project);
            Ok(simulation)
        } else {
            Ok(Self::new(registry, AssetLibrary::open(&root)))
        }
    }

    /// The project the run opened, when it is in one.
    pub fn project(&self) -> Option<&Project> {
        self.project.as_ref()
    }

    /// The scene a run starts in: the option's when given, otherwise the
    /// project's.
    pub fn startup_scene(&self, options: &RunOptions) -> Option<std::path::PathBuf> {
        if !options.startup_scene.as_os_str().is_empty() {
            return Some(options.startup_scene.clone());
        }
        self.project
            .as_ref()
            .and_then(Project::startup_scene)
            .map(std::path::PathBuf::from)
    }

    /// The borrowed objects the runtime works on.
    pub fn services(&mut self) -> Services<'_> {
        Services {
            scene: &mut self.scene,
            assets: &mut self.assets,
            input: &self.input,
        }
    }

    /// The context an application callback receives.
    pub fn context(&mut self) -> AppContext<'_> {
        AppContext {
            scene: &mut self.scene,
            runtime: &mut self.runtime,
            assets: &mut self.assets,
            input: &self.input,
        }
    }

    /// Loads the scene at a path relative to the asset root, replacing the
    /// loaded one at once.
    ///
    /// # Errors
    ///
    /// When the scene cannot be read, does not validate or does not realize.
    pub fn load_scene(&mut self, path: &Path) -> EngineResult {
        let asset = self.assets.scene_at(path)?;
        let mut services = Services {
            scene: &mut self.scene,
            assets: &mut self.assets,
            input: &self.input,
        };
        self.runtime.load(&mut services, &asset)
    }

    /// Calls the application's `start`.
    ///
    /// # Errors
    ///
    /// The reason the application cannot start.
    pub fn start<A: App>(&mut self, app: &mut A) -> EngineResult {
        app.start(&mut self.context())
    }

    /// Runs one fixed step: the application's fixed update, the behaviours'
    /// fixed updates, then the physics step.
    ///
    /// # Errors
    ///
    /// The failure of the application or of a behaviour.
    pub fn fixed_update<A: App>(&mut self, app: &mut A, info: &FixedFrameInfo) -> EngineResult {
        app.fixed_update(&mut self.context(), info)?;
        let mut services = Services {
            scene: &mut self.scene,
            assets: &mut self.assets,
            input: &self.input,
        };
        self.runtime.fixed_update(&mut services, info)?;
        self.scene.step_physics(info.delta_seconds);
        Ok(())
    }

    /// Runs the per-frame updates: the application's, then the behaviours'.
    ///
    /// # Errors
    ///
    /// The failure of the application or of a behaviour.
    pub fn update<A: App>(&mut self, app: &mut A, info: &FrameInfo) -> EngineResult {
        app.update(&mut self.context(), info)?;
        let mut services = Services {
            scene: &mut self.scene,
            assets: &mut self.assets,
            input: &self.input,
        };
        self.runtime.update(&mut services, info)
    }

    /// Applies the destruction, spawning and scene replacement the frame
    /// queued: the single structural boundary.
    ///
    /// # Errors
    ///
    /// The failure of a behaviour or of a scene replacement.
    pub fn structural_boundary(&mut self) -> EngineResult {
        let mut services = Services {
            scene: &mut self.scene,
            assets: &mut self.assets,
            input: &self.input,
        };
        self.runtime.structural_boundary(&mut services)
    }

    /// Resolves the transforms and applies the interpolation of the bodies: the
    /// last two calls before the frame is described.
    pub fn prepare_frame(&mut self, interpolation: f32) {
        self.scene.resolve_transforms();
        self.scene.apply_transform_interpolation(interpolation);
    }

    /// Tears the behaviours down; call it before dropping the simulation.
    ///
    /// # Errors
    ///
    /// The first failure of a behaviour while it is torn down.
    pub fn shutdown(&mut self) -> EngineResult {
        let mut services = Services {
            scene: &mut self.scene,
            assets: &mut self.assets,
            input: &self.input,
        };
        self.runtime.teardown(&mut services)
    }
}
