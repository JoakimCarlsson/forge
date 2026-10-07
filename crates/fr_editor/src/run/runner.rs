//! The state machine behind the Play and Build commands.

use std::path::Path;

use fr_authoring::{LaunchCommand, LaunchPlan};

use super::build_job::BuildJob;
use super::output::{RunEvent, notice};
use super::process::GameProcess;

/// What the runner is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    /// Nothing is running.
    Idle,
    /// The build is running, and the game starts after it when asked to.
    Building,
    /// The game process is running.
    Running,
}

/// Plans a run from a project directory and a scene. The default planner opens
/// the project and calls `fr_authoring::launch_plan`; a check can substitute
/// any commands.
pub type Planner = Box<dyn Fn(&Path, &Path) -> Result<LaunchPlan, String> + Send>;

/// The planner that opens the project and plans with `fr_authoring`.
fn project_plan(project: &Path, scene: &Path) -> Result<LaunchPlan, String> {
    let project = fr_project::Project::open(project).map_err(|error| error.to_string())?;
    fr_authoring::launch_plan(&project, scene).map_err(|error| error.to_string())
}

/// Builds and runs a project's game as a separate process, one thing at a time.
pub struct Runner {
    /// How a run is planned.
    planner: Planner,
    /// The build in progress.
    job: Option<BuildJob>,
    /// The game process in progress.
    process: Option<GameProcess>,
    /// What to start when the build in progress succeeds.
    launch_after: Option<LaunchCommand>,
}

impl Default for Runner {
    /// A runner that plans with the project and `fr_authoring`.
    fn default() -> Self {
        Self::with_planner(Box::new(project_plan))
    }
}

impl Runner {
    /// A runner that plans every run with the given planner, so a headless
    /// check can run any commands through the same path.
    pub fn with_planner(planner: Planner) -> Self {
        Self {
            planner,
            job: None,
            process: None,
            launch_after: None,
        }
    }

    /// What the runner is doing.
    pub fn state(&self) -> RunState {
        if self.job.is_some() {
            RunState::Building
        } else if self.process.is_some() {
            RunState::Running
        } else {
            RunState::Idle
        }
    }

    /// Fails when the save failed or something is already running.
    fn check_ready(&self, save_result: Result<(), String>) -> Result<(), String> {
        save_result.map_err(|error| format!("saving failed, nothing was started: {error}"))?;
        if self.state() == RunState::Idle {
            Ok(())
        } else {
            Err("already building or running".to_owned())
        }
    }

    /// Plays the project on a scene (absolute or relative to the asset root).
    /// With `build_first` the build runs and the game starts when it succeeds;
    /// otherwise the game starts at once. Progress arrives through
    /// [`poll`](Self::poll).
    ///
    /// # Errors
    ///
    /// The reason, as text, when the save failed (nothing is spawned), when
    /// something is already running, or when the run cannot be planned or
    /// started.
    pub fn play(
        &mut self,
        project: &Path,
        scene: &Path,
        build_first: bool,
        save_result: Result<(), String>,
    ) -> Result<(), String> {
        self.check_ready(save_result)?;
        let plan = (self.planner)(project, scene)?;
        if build_first {
            self.job = Some(BuildJob::start(plan.build));
            self.launch_after = Some(plan.launch);
            Ok(())
        } else {
            self.process = Some(GameProcess::spawn(&plan.launch)?);
            Ok(())
        }
    }

    /// Builds the project without starting the game.
    ///
    /// # Errors
    ///
    /// The reason, as text, when the save failed, something is already running
    /// or the build cannot be planned.
    pub fn build(&mut self, project: &Path, save_result: Result<(), String>) -> Result<(), String> {
        self.check_ready(save_result)?;
        let plan = (self.planner)(project, Path::new(""))?;
        self.job = Some(BuildJob::start(plan.build));
        Ok(())
    }

    /// Cancels the build and kills the game, returning the remaining output and
    /// the exit event.
    pub fn stop(&mut self) -> Vec<RunEvent> {
        let mut events = Vec::new();
        if let Some(job) = self.job.take() {
            job.cancel();
            events.push(notice("build cancelled"));
        }
        self.launch_after = None;
        if let Some(mut process) = self.process.take() {
            events.extend(process.stop());
            events.push(notice("game stopped"));
        }
        events
    }

    /// Everything that happened since the last call, in order. Never blocks.
    pub fn poll(&mut self) -> Vec<RunEvent> {
        let mut events = self.poll_build();
        events.extend(self.poll_game());
        events
    }

    /// The build's output and, when it ends, what follows it.
    fn poll_build(&mut self) -> Vec<RunEvent> {
        let Some(job) = self.job.as_mut() else {
            return Vec::new();
        };
        let mut events = job.poll();
        let finished = events.iter().find_map(|event| match event {
            RunEvent::BuildFinished { ok } => Some(*ok),
            _ => None,
        });
        if let Some(ok) = finished {
            self.job = None;
            events.extend(self.after_build(ok));
        }
        events
    }

    /// The messages after a build ended, starting the game when it succeeded
    /// and one is waiting.
    fn after_build(&mut self, ok: bool) -> Vec<RunEvent> {
        let launch = self.launch_after.take();
        if !ok {
            return vec![notice("build failed")];
        }
        let mut events = vec![notice("build finished")];
        if let Some(launch) = launch {
            match GameProcess::spawn(&launch) {
                Ok(process) => {
                    self.process = Some(process);
                    events.push(notice(format!("running {}", launch.display())));
                }
                Err(error) => events.push(notice(error)),
            }
        }
        events
    }

    /// The game's output and, when it ends, a message about how.
    fn poll_game(&mut self) -> Vec<RunEvent> {
        let Some(process) = self.process.as_mut() else {
            return Vec::new();
        };
        let mut events = process.poll();
        let exit = events.iter().find_map(|event| match event {
            RunEvent::Exited { code, .. } => Some(*code),
            _ => None,
        });
        if let Some(code) = exit {
            self.process = None;
            events.push(notice(match code {
                Some(code) => format!("game exited with code {code}"),
                None => "game was terminated".to_owned(),
            }));
        }
        events
    }
}
