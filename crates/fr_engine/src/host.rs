//! The frame loop's host: glue between the application, the simulation, the
//! window, the renderer and the UI. The order of a frame is written here once.

use fr_color::Rgba;
use fr_input::{ButtonState, KeyEvent, PointerButton, ScrollDelta};
use fr_math::{Point, Size};
use fr_render::{DrawList, Renderer, Scene as RenderScene};
use fr_scene::{GpuResources, ResourceCache};
use fr_time::{FixedStepper, FrameClock};
use fr_ui::{Theme, Ui};
use fr_window::{Window, WindowHandler};

use crate::app::App;
use crate::error::EngineError;
use crate::simulation::Simulation;
use crate::timing::{FixedFrameInfo, FrameInfo};

/// The most fixed steps one frame may run before the remaining time is dropped.
const MAX_FIXED_STEPS_PER_FRAME: u32 = 8;

/// Connects an [`App`] to the window, the clock, the renderer and the UI.
pub(crate) struct Host<'a, A: App> {
    /// The game being driven.
    app: &'a mut A,
    /// The scene, runtime, assets and input.
    pub(crate) simulation: Simulation,
    /// The renderer, once the window exists.
    renderer: Option<Renderer>,
    /// What is on the graphics device.
    cache: ResourceCache,
    /// The UI state that outlives each frame's tree.
    ui: Ui<A::Message>,
    /// The primitives of the frame being built, reused from frame to frame.
    list: DrawList,
    /// The 3D frame the stage describes, reused from frame to frame.
    frame: RenderScene,
    /// Times each frame.
    clock: FrameClock,
    /// Turns frame times into fixed steps.
    stepper: FixedStepper,
    /// The number of fixed steps run so far.
    fixed_steps: u64,
    /// The current drawable size in physical pixels.
    size: (u32, u32),
    /// The physical pixels per logical pixel.
    scale_factor: f64,
    /// The frames after which the loop ends; zero for never.
    max_frames: u64,
    /// The first failure, reported when the loop ends.
    pub(crate) failure: Option<EngineError>,
}

impl<'a, A: App> Host<'a, A> {
    /// A host that drives `app` over `simulation`.
    pub(crate) fn new(app: &'a mut A, simulation: Simulation, max_frames: u64) -> Self {
        Self {
            app,
            simulation,
            renderer: None,
            cache: ResourceCache::default(),
            ui: Ui::new(Theme::default()),
            list: DrawList::new(Size::zero()),
            frame: RenderScene::default(),
            clock: FrameClock::new(),
            stepper: FixedStepper::new(1.0 / 60.0, MAX_FIXED_STEPS_PER_FRAME),
            fixed_steps: 0,
            size: (0, 0),
            scale_factor: 1.0,
            max_frames,
            failure: None,
        }
    }

    /// Keeps the first failure; the loop ends at the end of the frame.
    fn fail(&mut self, error: impl Into<EngineError>) {
        if self.failure.is_none() {
            self.failure = Some(error.into());
        }
    }

    /// Passes the renderer the latest size and scale factor.
    fn resize_renderer(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(self.size.0, self.size.1, self.scale_factor as f32);
        }
    }

    /// Applies the message a control sent, if it sent one.
    fn deliver(&mut self, message: Option<A::Message>) {
        if let Some(message) = message
            && let Err(error) = self.app.message(&mut self.simulation.context(), message)
        {
            self.fail(error);
        }
    }

    /// Runs the fixed steps the frame owes and answers how far the frame is
    /// into the next one. A game that does not simulate runs none and draws the
    /// bodies where the last step left them.
    fn run_fixed_steps(&mut self, delta_seconds: f32) -> Result<f32, EngineError> {
        if !self.app.simulates_physics() {
            return Ok(1.0);
        }
        let timestep = self.app.fixed_timestep();
        if timestep <= 0.0 {
            return Ok(1.0);
        }
        if self.stepper.step() != timestep {
            self.stepper.set_step(timestep);
        }
        for _ in 0..self.stepper.advance(delta_seconds) {
            let info = FixedFrameInfo {
                index: self.fixed_steps,
                delta_seconds: timestep,
            };
            self.simulation.fixed_update(&mut *self.app, &info)?;
            self.fixed_steps += 1;
        }
        Ok(self.stepper.alpha())
    }

    /// Runs one frame: fixed steps, updates, the UI, the structural boundary,
    /// transform resolution and interpolation, then the draw.
    fn run_frame(&mut self) -> Result<(), EngineError> {
        self.clock.tick();
        let delta_seconds = self.clock.delta_seconds();
        let interpolation = self.run_fixed_steps(delta_seconds)?;
        let info = FrameInfo {
            delta_seconds,
            index: self.clock.frame(),
            width: self.size.0,
            height: self.size.1,
            interpolation,
        };
        self.simulation.update(&mut *self.app, &info)?;
        self.simulation.input.end_frame();
        self.build_ui();
        self.simulation.structural_boundary()?;
        self.simulation.prepare_frame(interpolation);
        self.draw()
    }

    /// Builds this frame's UI tree and paints it into the draw list.
    fn build_ui(&mut self) {
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let theme = *self.ui.theme();
        let viewport = renderer.size();
        self.list.reset(viewport);
        let view = self.app.view(&theme);
        self.ui.draw(
            renderer.text(),
            &mut self.list,
            viewport,
            Point::default(),
            view,
        );
    }

    /// Describes the stage as a 3D frame and hands it and the UI to the renderer.
    fn draw(&mut self) -> Result<(), EngineError> {
        let Some(renderer) = &mut self.renderer else {
            return Ok(());
        };
        let vsync = self.app.vsync();
        if renderer.vsync() != vsync {
            renderer.set_vsync(vsync);
        }
        let mut resources = GpuResources {
            renderer,
            assets: &mut self.simulation.assets,
            cache: &mut self.cache,
        };
        self.simulation
            .scene
            .build_render_scene(&mut resources, &mut self.frame)?;
        let clear = self.simulation.scene.clear_color();
        renderer.render(
            &self.list,
            Some(&self.frame),
            Rgba::new(clear.r, clear.g, clear.b, clear.a),
        );
        Ok(())
    }
}

impl<A: App> WindowHandler for Host<'_, A> {
    /// Creates the renderer for the new window.
    fn created(&mut self, window: &Window) {
        self.size = window.size();
        self.scale_factor = window.scale_factor();
        match Renderer::new(
            window.clone(),
            self.size.0,
            self.size.1,
            self.scale_factor as f32,
        ) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => self.fail(error),
        }
    }

    /// Resizes the swapchain.
    fn resized(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        self.resize_renderer();
    }

    /// Rescales the swapchain's logical size.
    fn scale_factor_changed(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.resize_renderer();
    }

    /// Runs one frame, keeping the failure that ends the loop.
    fn redraw(&mut self) {
        if self.renderer.is_none() || self.failure.is_some() {
            return;
        }
        if let Err(error) = self.run_frame() {
            self.fail(error);
        }
    }

    /// Moves the pointer in the UI and the input state.
    fn pointer_moved(&mut self, x: f32, y: f32) {
        let message = self.ui.pointer_moved(Point::new(x, y));
        self.deliver(message);
        self.simulation.input.pointer_moved(x, y);
    }

    /// Takes the pointer out of the UI and the input state.
    fn pointer_left(&mut self) {
        self.ui.pointer_left();
        self.simulation.input.pointer_left();
    }

    /// Presses or releases a button in the UI, and tells the game what the UI
    /// did not take: a press over the UI never reaches the game, a release
    /// always does, so a drag is never left half done.
    fn pointer_button(&mut self, button: PointerButton, state: ButtonState) {
        let over_ui = self.ui.pointer_over_region();
        let message = self.ui.pointer_button(button, state);
        self.deliver(message);
        if state == ButtonState::Released || !over_ui {
            self.simulation.input.button(button, state);
        }
    }

    /// Gives the wheel to the scroll area under the pointer, and tells the game
    /// about scrolling that is not over the UI.
    fn scrolled(&mut self, delta: ScrollDelta) {
        let message = self.ui.scrolled(delta);
        self.deliver(message);
        if !self.ui.pointer_over_region() {
            self.simulation.input.scrolled(delta);
        }
    }

    /// Records the key for the game, then moves focus on tab, activates it on
    /// enter or space, drops it on escape.
    fn key(&mut self, event: &KeyEvent) {
        self.simulation.input.key(event.clone());
        let message = self.ui.key(event);
        self.deliver(message);
    }

    /// Ignores text, which no UI element takes yet.
    fn text_input(&mut self, _text: &str) {}

    /// Whether a failure, the frame limit or the game ends the loop.
    fn wants_exit(&self) -> bool {
        self.failure.is_some()
            || self.app.wants_exit()
            || (self.max_frames > 0 && self.clock.frame() >= self.max_frames)
    }
}
