//! The game-facing ECS builder and the window-independent execution lifecycle.

use fr_app::ecs as bevy_ecs;
use fr_app::ecs::reflect::AppTypeRegistry;
use fr_app::ecs::resource::Resource;
use fr_app::ecs::schedule::{
    ExecutorKind, InternedScheduleLabel, IntoScheduleConfigs, Schedule, ScheduleLabel,
};
use fr_app::ecs::system::ScheduleSystem;
use fr_app::ecs::world::World;
use fr_app::reflect::GetTypeRegistration;
use fr_app::{
    First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last, Plugins,
    PostStartup, PostUpdate, PreStartup, PreUpdate, Runtime, Startup, Update,
};
use fr_color::Rgba;
use fr_input::CursorMode;
use fr_render::Scene;
use fr_scene::ScenePlugin;
use fr_time::FixedStepper;
use fr_ui::{Div, Theme, div};

use crate::extraction::{RenderSystems, extract_scene};
use crate::{Assets, EngineError, FixedStep, Frame, Input, RenderScene};

/// The most fixed steps one frame may run before remaining time is dropped.
const MAX_FIXED_STEPS: u32 = 8;

/// The stage that constructs the renderer's frame after world updates.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Extract;

/// Window and simulation settings that systems can change during play.
#[derive(Resource, Clone, Debug)]
pub struct WindowSettings {
    /// Whether presenting waits for the display refresh.
    pub vsync: bool,
    /// Whether the pointer is captured by the window.
    pub cursor_mode: CursorMode,
    /// Seconds per fixed step; zero disables fixed updates.
    pub fixed_timestep: f32,
}

impl Default for WindowSettings {
    /// Uses an uncapped window and a 60 Hz fixed simulation.
    fn default() -> Self {
        Self {
            vsync: false,
            cursor_mode: CursorMode::Normal,
            fixed_timestep: 1.0 / 60.0,
        }
    }
}

/// Input accumulated since the previous frame, available through every update stage.
#[derive(Resource, Default)]
pub struct Inputs(pub Vec<Input>);

/// UI messages accumulated since the previous frame.
#[derive(Resource)]
pub struct UiMessages<M: Send + Sync + 'static>(pub Vec<M>);

/// An application failure reported by a system and returned by the engine host.
#[derive(Resource, Default)]
pub struct AppFailure(pub Option<String>);

/// A pure function that constructs the current UI from world state.
type View<M> = fn(&World, &Theme) -> Div<M>;

/// A pure function that selects the window clear colour.
type ClearColor = fn(&World, &Theme) -> Rgba;

/// An ECS application composed from resources, systems and plugins.
///
/// The runtime can execute without a window through [`Self::start`] and
/// [`Self::update`]. The UI is rebuilt from world state and emits messages.
pub struct App<M: Clone + Send + Sync + 'static = ()> {
    /// The ECS runtime and its plugin registry.
    pub(crate) runtime: Runtime,
    /// The current UI constructor.
    pub(crate) view: View<M>,
    /// The window clear colour selector.
    pub(crate) clear_color: ClearColor,
    /// Whether startup has run.
    started: bool,
    /// Accumulates fixed simulation time.
    stepper: FixedStepper,
    /// The number of fixed steps already executed.
    fixed_steps: u64,
}

impl<M: Clone + Send + Sync + 'static> Default for App<M> {
    /// Creates a runtime with scene, asset and frame resources.
    fn default() -> Self {
        let mut runtime = Runtime::new();
        for label in [
            PreStartup.intern(),
            Startup.intern(),
            PostStartup.intern(),
            First.intern(),
            PreUpdate.intern(),
            FixedFirst.intern(),
            FixedPreUpdate.intern(),
            FixedUpdate.intern(),
            FixedPostUpdate.intern(),
            FixedLast.intern(),
            Update.intern(),
            PostUpdate.intern(),
            Last.intern(),
        ] {
            runtime.init_schedule(label);
            if let Some(schedule) = runtime.get_schedule_mut(label) {
                schedule.set_executor_kind(ExecutorKind::SingleThreaded);
            }
        }
        runtime
            .add_plugins(ScenePlugin)
            .init_resource::<Assets>()
            .init_resource::<WindowSettings>()
            .init_resource::<Inputs>()
            .init_resource::<AppFailure>()
            .insert_resource(UiMessages::<M>(Vec::new()))
            .insert_resource(RenderScene(Scene::default()))
            .insert_resource(Frame {
                delta_seconds: 0.0,
                index: 0,
                width: 1,
                height: 1,
                scale_factor: 1.0,
                interpolation: 0.0,
            })
            .insert_resource(FixedStep {
                index: 0,
                delta_seconds: 1.0 / 60.0,
            })
            .add_schedule(Schedule::new(Extract))
            .add_systems(Extract, extract_scene.in_set(RenderSystems::Extract));
        Self {
            runtime,
            view: empty_view::<M>,
            clear_color: background,
            started: false,
            stepper: FixedStepper::new(1.0 / 60.0, MAX_FIXED_STEPS),
            fixed_steps: 0,
        }
    }
}

impl<M: Clone + Send + Sync + 'static> App<M> {
    /// Creates an application with the default engine resources.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the ECS world for inspection or direct entity access.
    pub fn world(&self) -> &World {
        self.runtime.world()
    }

    /// Returns the ECS world for resource and entity changes.
    pub fn world_mut(&mut self) -> &mut World {
        self.runtime.world_mut()
    }

    /// Returns the underlying builder for advanced schedule and plugin configuration.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    /// Adds one plugin or a tuple of plugins.
    pub fn add_plugins<P>(&mut self, plugins: impl Plugins<P>) -> &mut Self {
        self.runtime.add_plugins(plugins);
        self
    }

    /// Adds systems to a standard or application-defined schedule.
    pub fn add_systems<S>(
        &mut self,
        schedule: impl ScheduleLabel,
        systems: impl IntoScheduleConfigs<ScheduleSystem, S>,
    ) -> &mut Self {
        self.runtime.add_systems(schedule, systems);
        self
    }

    /// Inserts a resource shared by systems.
    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.runtime.insert_resource(resource);
        self
    }

    /// Registers reflection metadata for inspectors and scene serialization.
    pub fn register_type<T: GetTypeRegistration>(&mut self) -> &mut Self {
        self.runtime.register_type::<T>();
        self
    }

    /// Sets the UI constructor and clear colour selector.
    pub fn set_view(&mut self, view: View<M>, clear_color: ClearColor) -> &mut Self {
        self.view = view;
        self.clear_color = clear_color;
        self
    }

    /// Finishes plugins and runs startup schedules exactly once.
    ///
    /// # Errors
    ///
    /// Returns an application failure reported by startup systems.
    pub fn start(&mut self) -> Result<(), EngineError> {
        if !self.started {
            self.runtime.finish();
            self.runtime.cleanup();
            self.started = true;
            self.run_stages(&[PreStartup.intern(), Startup.intern(), PostStartup.intern()])?;
        }
        self.check_failure()
    }

    /// Runs input, fixed simulation, frame updates and render extraction in order.
    ///
    /// # Errors
    ///
    /// Returns an application failure or invalid timing settings.
    pub fn update(&mut self, mut frame: Frame) -> Result<(), EngineError> {
        self.start()?;
        let timestep = self.world().resource::<WindowSettings>().fixed_timestep;
        if !timestep.is_finite()
            || timestep < 0.0
            || !frame.delta_seconds.is_finite()
            || frame.delta_seconds < 0.0
        {
            return Err(EngineError::App(String::from(
                "frame and fixed-step durations must be finite and nonnegative",
            )));
        }
        if self.stepper.step() != timestep {
            self.stepper.set_step(timestep);
        }
        let steps = self.stepper.advance(frame.delta_seconds);
        frame.interpolation = self.stepper.alpha();
        self.world_mut().insert_resource(frame);
        self.run_stages(&[First.intern(), PreUpdate.intern()])?;
        for _ in 0..steps {
            let step = FixedStep {
                index: self.fixed_steps,
                delta_seconds: timestep,
            };
            self.world_mut().insert_resource(step);
            self.run_stages(&[
                FixedFirst.intern(),
                FixedPreUpdate.intern(),
                FixedUpdate.intern(),
                FixedPostUpdate.intern(),
                FixedLast.intern(),
            ])?;
            self.fixed_steps += 1;
        }
        self.run_stages(&[Update.intern(), PostUpdate.intern(), Last.intern()])?;
        self.world_mut().resource_mut::<RenderScene>().0.clear();
        self.run_stages(&[Extract.intern()])?;
        self.world_mut().resource_mut::<Inputs>().0.clear();
        self.world_mut().resource_mut::<UiMessages<M>>().0.clear();
        self.check_failure()
    }

    /// Runs ordered stages and stops when a system reports an application failure.
    fn run_stages(&mut self, stages: &[InternedScheduleLabel]) -> Result<(), EngineError> {
        for &stage in stages {
            self.world_mut().run_schedule(stage);
            self.check_failure()?;
        }
        Ok(())
    }

    /// Returns a failure recorded by an application system.
    fn check_failure(&self) -> Result<(), EngineError> {
        match &self.world().resource::<AppFailure>().0 {
            Some(reason) => Err(EngineError::App(reason.clone())),
            None => Ok(()),
        }
    }

    /// Loads and instantiates a scene document from disk.
    ///
    /// # Errors
    ///
    /// Returns an error for file failures, invalid scenes or conflicting object IDs.
    pub fn load_scene(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<
        std::collections::BTreeMap<fr_scene::SceneId, fr_app::ecs::entity::Entity>,
        fr_scene::SceneError,
    > {
        let source =
            fr_assets::load_text(path).map_err(|error| fr_scene::SceneError(error.to_string()))?;
        let document = fr_scene::SceneDocument::from_json(&source)?;
        self.spawn_scene(&document)
    }

    /// Captures reflected authored objects and saves their scene document.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid component data or a file write failure.
    pub fn save_scene(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), fr_scene::SceneError> {
        let source = self.capture_scene()?.to_json()?;
        fr_assets::save_text(path, &source).map_err(|error| fr_scene::SceneError(error.to_string()))
    }

    /// Captures reflected authored objects as a scene document.
    ///
    /// # Errors
    ///
    /// Returns an error if component data or the authored hierarchy is invalid.
    pub fn capture_scene(&mut self) -> Result<fr_scene::SceneDocument, fr_scene::SceneError> {
        let registry = self.world().resource::<AppTypeRegistry>().clone();
        let registry = registry.read();
        fr_scene::SceneDocument::capture(self.world_mut(), &registry)
    }

    /// Instantiates authored objects after validating their reflected components.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data, unknown components or conflicting IDs.
    pub fn spawn_scene(
        &mut self,
        scene: &fr_scene::SceneDocument,
    ) -> Result<
        std::collections::BTreeMap<fr_scene::SceneId, fr_app::ecs::entity::Entity>,
        fr_scene::SceneError,
    > {
        let registry = self.world().resource::<AppTypeRegistry>().clone();
        let registry = registry.read();
        scene.instantiate(self.world_mut(), &registry)
    }
}

/// Constructs an empty UI tree.
fn empty_view<M: Clone + 'static>(_world: &World, _theme: &Theme) -> Div<M> {
    div()
}

/// Selects the theme's background colour.
fn background(_world: &World, theme: &Theme) -> Rgba {
    theme.colors.background
}
