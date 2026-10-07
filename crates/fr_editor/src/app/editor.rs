//! The editor as one object: input in, a frame out.
//!
//! The window host and the headless capture drive the same [`Editor`]: they
//! call the input methods below with window events or synthetic ones, and
//! `frame` once per frame. A frame drains the runner into the console, brings
//! the preview up to date, gives the viewport the input the interface did not
//! take, builds and draws the interface, and renders the 3D frame into the
//! open middle of the dock with the interface over it.

use std::path::{Path, PathBuf};

use fr_input::{ButtonState, Key, KeyEvent, PointerButton, ScrollDelta};
use fr_math::{Point, Rect};
use fr_render::{Capture, DrawList, Renderer, Scene};
use fr_ui::{Theme, Ui};

use super::actions::{absorb, perform};
use super::feed::ViewportFeed;
use super::layout::{load_layout, save_layout};
use super::message::Message;
use super::picker::open_now;
use super::project_open::save_project_state;
use super::state::EditorState;
use super::text_input;
use super::update::{close_menus, update};
use super::{edit, shortcuts, view};
use crate::options::{EditorOptions, ToolChoice};
use crate::panels::console::ConsoleLevel;
use crate::settings::Settings;
use crate::viewport::Tool;

/// The wheel distance that counts as one notch.
const NOTCH_PIXELS: f32 = 40.0;

/// The longest frame time the viewport is told about, in seconds.
const MAX_DELTA: f32 = 0.1;

/// The whole editor.
pub struct Editor {
    /// Everything the editor shows and edits.
    pub state: EditorState,
    /// The interface's pointer, focus and regions.
    ui: Ui<Message>,
    /// The interface of the frame being built.
    list: DrawList,
    /// The 3D frame, reused from frame to frame.
    scene: Scene,
    /// The renderer, once there is a window or an offscreen target.
    renderer: Option<Renderer>,
    /// What the window's events left for the viewport.
    feed: ViewportFeed,
    /// The last failure to describe the 3D frame, so it is reported once.
    last_frame_error: Option<String>,
    /// The preview's messages last put in the console, so a rebuild that finds
    /// the same things does not repeat them.
    reported_diagnostics: Vec<String>,
}

/// The settings of the current user, or defaults when they cannot be read.
fn load_settings() -> (Settings, Option<String>) {
    match Settings::load() {
        Ok(settings) => (settings, None),
        Err(error) => {
            let directory = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("forge-editor");
            (Settings::in_directory(directory), Some(error))
        }
    }
}

impl Editor {
    /// An editor over a state, with nothing drawn yet.
    pub fn new(state: EditorState) -> Self {
        Self {
            state,
            ui: Ui::new(Theme::default()),
            list: DrawList::new(fr_math::Size::zero()),
            scene: Scene::default(),
            renderer: None,
            feed: ViewportFeed::default(),
            last_frame_error: None,
            reported_diagnostics: Vec::new(),
        }
    }

    /// Builds the editor the command line asks for: the project is the
    /// argument, else the last one, else the picker; `persist` says whether
    /// settings, layout and project state are written.
    ///
    /// # Errors
    ///
    /// The message of the project, scene or entity the options name that cannot
    /// be opened or found.
    pub fn start(options: &EditorOptions, persist: bool) -> Result<Self, String> {
        let (settings, settings_error) = load_settings();
        let mut state = EditorState::new(settings);
        state.persist = persist;
        if let Some(error) = settings_error {
            state.error(format!("cannot read the settings: {error}"));
        }
        if persist {
            load_layout(&mut state);
        }
        let last = state.settings.last_project().map(Path::to_path_buf);
        match (&options.project, last) {
            _ if options.picker => state.show_picker(),
            (Some(project), _) => super::project_open::open_project(&mut state, project)?,
            (None, Some(last)) => open_now(&mut state, &last),
            (None, None) => state.show_picker(),
        }
        if state.project.is_none() && state.picker.is_none() {
            state.show_picker();
        }
        let mut editor = Self::new(state);
        editor.apply_options(options)?;
        Ok(editor)
    }

    /// Opens the scene, selects the entity and chooses the tool the options
    /// name.
    ///
    /// # Errors
    ///
    /// The message of the scene that cannot be opened or the entity that is not
    /// in it.
    pub fn apply_options(&mut self, options: &EditorOptions) -> Result<(), String> {
        let state = &mut self.state;
        if let (Some(scene), true) = (&options.scene, state.has_project()) {
            let path = if scene.is_absolute() {
                scene.clone()
            } else {
                state.workspace.asset_root().join(scene)
            };
            state
                .workspace
                .open(&path)
                .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
        }
        if let Some(name) = &options.select {
            select_by_name(state, name)?;
        }
        state.viewport.set_tool(match options.tool {
            ToolChoice::Select => Tool::Select,
            ToolChoice::Move => Tool::Move,
            ToolChoice::Rotate => Tool::Rotate,
            ToolChoice::Scale => Tool::Scale,
        });
        Ok(())
    }

    /// Gives the editor the renderer it draws with.
    pub fn attach_renderer(&mut self, renderer: Renderer) {
        self.renderer = Some(renderer);
    }

    /// Whether a renderer is attached.
    pub fn has_renderer(&self) -> bool {
        self.renderer.is_some()
    }

    /// Resizes the target to a physical size at a scale factor.
    pub fn resize(&mut self, width: u32, height: u32, scale: f32) {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(width, height, scale);
        }
    }

    /// Reads the frame just rendered back, for an offscreen renderer.
    ///
    /// # Errors
    ///
    /// The message of the failure, or that there is no renderer.
    pub fn capture(&mut self) -> Result<Capture, String> {
        self.renderer
            .as_mut()
            .ok_or_else(|| "there is no renderer to capture".to_owned())?
            .capture()
            .map_err(|error| error.to_string())
    }

    /// Whether the pointer is over the 3D view and nothing of the interface is
    /// over the pointer or holding it.
    fn pointer_over_viewport(&self) -> bool {
        let state = &self.state;
        let Some(pointer) = self.ui.pointer() else {
            return false;
        };
        let blocked = state.picker.is_some()
            || state.popup.is_some()
            || state.prompt.is_some()
            || state.menu.open.is_some()
            || state.dock_drag.is_some()
            || !state.has_project();
        !blocked
            && state.viewport_rect.contains(pointer)
            && !self.ui.pointer_over_region()
            && !self.ui.dragging()
    }

    /// Applies the message an input produced, if it produced one.
    fn deliver(&mut self, message: Option<Message>) {
        if let Some(message) = message {
            update(&mut self.state, message);
        }
    }

    /// The pointer moved to a point of the window, in logical pixels.
    pub fn pointer_moved(&mut self, x: f32, y: f32) {
        let point = Point::new(x, y);
        self.feed.moved(point);
        self.state.pointer = point;
        let message = self.ui.pointer_moved(point);
        self.deliver(message);
    }

    /// The pointer left the window.
    pub fn pointer_left(&mut self) {
        self.feed.left();
        self.ui.pointer_left();
    }

    /// A pointer button went down or came up.
    pub fn pointer_button(&mut self, button: PointerButton, button_state: ButtonState) {
        if let Some(pointer) = self.ui.pointer() {
            self.state.pointer = pointer;
        }
        let over_viewport = self.pointer_over_viewport();
        let message = self.ui.pointer_button(button, button_state);
        self.state.modifiers = self.ui.modifiers();
        match button_state {
            ButtonState::Pressed => {
                if !matches!(message, Some(Message::Text(_))) {
                    text_input::blur(&mut self.state);
                }
                if over_viewport {
                    self.state.focused_panel = None;
                    self.feed.press(button);
                }
            }
            ButtonState::Released => self.feed.release(button),
        }
        self.deliver(message);
    }

    /// The wheel turned.
    pub fn scrolled(&mut self, delta: ScrollDelta) {
        let message = self.ui.scrolled(delta);
        if message.is_some() {
            self.deliver(message);
        } else if self.pointer_over_viewport() {
            self.feed.scrolled(delta.y / NOTCH_PIXELS);
        }
    }

    /// Closes what Escape closes first: a question, a menu, a popup, the
    /// picker over an open project.
    fn close_overlays(&mut self) -> bool {
        let state = &mut self.state;
        if state.prompt.take().is_some() {
            return true;
        }
        if state.popup.is_some() || state.menu.open.is_some() {
            close_menus(state);
            return true;
        }
        if state.picker.is_some() && state.has_project() && state.text.target.is_none() {
            state.picker = None;
            return true;
        }
        false
    }

    /// A key went down or came up.
    pub fn key(&mut self, event: &KeyEvent) {
        self.state.modifiers = event.modifiers;
        let pressed = event.state == ButtonState::Pressed;
        if pressed {
            if event.key == Key::Escape && self.close_overlays() {
                return;
            }
            if text_input::key(&mut self.state, event) {
                return;
            }
            if self.state.prompt.is_none()
                && self.state.picker.is_none()
                && let Some(action) = shortcuts::action_for(event)
            {
                perform(&mut self.state, action);
                return;
            }
        }
        if self.state.text.target.is_none() && self.state.picker.is_none() {
            self.feed.key(event);
        }
        let message = self.ui.key(event);
        self.deliver(message);
    }

    /// Text a key press produced. Keys insert their text themselves, so this
    /// is not used.
    pub fn text_input(&mut self, _text: &str) {}

    /// Asks to close the window: the same as Quit, which may ask about unsaved
    /// changes first.
    pub fn close_requested(&mut self) {
        perform(&mut self.state, super::message::Action::Quit);
    }

    /// Whether the window should stay open: a question is waiting.
    pub fn keeps_open(&self) -> bool {
        !self.state.quit && self.state.prompt.is_some()
    }

    /// Writes what the editor remembers when it ends.
    pub fn shutdown(&mut self, window_size: Option<(u32, u32)>) {
        save_project_state(&mut self.state);
        if let Some(size) = window_size {
            self.state.settings.set_window_size(size);
        }
        self.state.persist_settings();
        save_layout(&mut self.state);
    }

    /// Runs one frame of `delta` seconds.
    pub fn frame(&mut self, delta: f32) {
        if self.renderer.is_none() {
            return;
        }
        let events = self.state.runner.poll();
        absorb(&mut self.state, events);
        self.sync_preview();
        self.update_viewport(delta.min(MAX_DELTA));
        self.draw_interface();
        self.render();
        save_layout(&mut self.state);
    }

    /// Brings the preview up to date with the current document.
    fn sync_preview(&mut self) {
        let state = &mut self.state;
        let Some(open) = state.workspace.current() else {
            state.preview.clear();
            return;
        };
        let messages = state.preview.sync(open.document(), &state.subsystems);
        if messages.is_empty() || messages == self.reported_diagnostics {
            return;
        }
        for message in &messages {
            state.log(ConsoleLevel::Warning, message.clone());
        }
        self.reported_diagnostics = messages;
    }

    /// Gives the viewport the frame's input.
    fn update_viewport(&mut self, delta: f32) {
        let hovered = self.pointer_over_viewport();
        let state = &mut self.state;
        if state.picker.is_some() || !state.has_project() {
            return;
        }
        if state.text.target.is_some() {
            self.feed.release_keys();
        }
        let input = self
            .feed
            .take(state.viewport_rect, hovered, state.modifiers, delta);
        let output = state.viewport.update(
            input,
            state.workspace.current_mut(),
            &state.preview,
            &state.subsystems,
        );
        for message in output.messages {
            state.error(message);
        }
    }

    /// Builds the interface and paints it into the draw list, behind it the
    /// viewport's overlay. When the open middle moved since the last frame the
    /// frame is painted once more with the new place.
    fn draw_interface(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let theme = *self.ui.theme();
        let logical = renderer.size();
        for pass in 0..2 {
            self.list.reset(logical);
            let rect = self.state.viewport_rect;
            let state = &self.state;
            if state.picker.is_none() && rect.size.width > 0.0 && rect.size.height > 0.0 {
                state.viewport.draw(
                    &mut self.list,
                    rect,
                    state.workspace.current(),
                    &state.preview,
                    &state.subsystems,
                );
            }
            let root = view::root(&self.state, &theme);
            self.ui.draw(
                renderer.text(),
                &mut self.list,
                logical,
                Point::default(),
                root,
            );
            let placed = view::viewport_rect(&self.state, &theme);
            let settled = placed == self.state.viewport_rect;
            self.state.viewport_rect = placed;
            if settled || pass == 1 {
                break;
            }
        }
    }

    /// Describes the 3D frame and renders it with the interface over it.
    fn render(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let state = &mut self.state;
        let rect: Rect = state.viewport_rect;
        let showing = state.picker.is_none()
            && state.has_project()
            && state.workspace.current().is_some()
            && rect.size.width > 0.0
            && rect.size.height > 0.0;
        if !showing {
            let background = self.ui.theme().colors.background;
            renderer.render_in(&self.list, None, None, background);
            return;
        }
        let camera = state.viewport.camera();
        match state
            .preview
            .build_frame(renderer, &camera, &mut self.scene)
        {
            Ok(()) => self.last_frame_error = None,
            Err(error) => {
                if self.last_frame_error.as_deref() != Some(error.as_str()) {
                    state.error(format!("cannot describe the frame: {error}"));
                    self.last_frame_error = Some(error);
                }
            }
        }
        let clear = state.preview.clear_color();
        renderer.render_in(&self.list, Some(&self.scene), Some(rect), clear);
    }
}

/// Selects the first entity or instance with a name in the current document.
///
/// # Errors
///
/// A message when no document is open or it has no node with the name.
fn select_by_name(state: &mut EditorState, name: &str) -> Result<(), String> {
    let found = state
        .document()
        .ok_or_else(|| format!("cannot select {name}: no scene is open"))?
        .document()
        .content()
        .ordered_nodes()
        .into_iter()
        .find(|node| {
            let content = state.document().map(|open| open.document().content());
            content.is_some_and(|content| {
                content
                    .find_entity(node.id)
                    .map(|entity| entity.name.as_str())
                    .or_else(|| {
                        content
                            .find_instance(node.id)
                            .map(|instance| instance.name.as_str())
                    })
                    == Some(name)
            })
        })
        .map(|node| node.id)
        .ok_or_else(|| format!("there is nothing named {name} in the scene"))?;
    edit::select(state, &[found]);
    Ok(())
}
