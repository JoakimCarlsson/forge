//! The viewport: camera navigation, picking and the transform gizmo tied
//! together over the preview.

use std::path::PathBuf;

use fr_authoring::OpenDocument;
use fr_camera::Camera;
use fr_math::{Aabb, Point, Rect, Vec3};
use fr_render::DrawList;

use super::editor_camera::EditorCamera;
use super::gizmo_draw::{draw_component_gizmos, draw_selection_outlines, draw_transform_gizmo};
use super::grid::draw_grid;
use super::input::{CursorHint, ViewportInput, ViewportKey, ViewportOutput};
use super::palette::ViewportPalette;
use super::picking::{apply_click, pick};
use super::projector::Projector;
use super::transform_gizmo::{Space, Tool, TransformGizmo};
use crate::preview::Preview;
use crate::subsystems::SubsystemRegistry;

/// How far the pointer may move between press and release, in logical pixels,
/// for the press to still count as a click.
const CLICK_SLOP: f32 = 4.0;

/// What the pointer is doing in the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    /// Nothing is held.
    Idle,
    /// Orbiting around the pivot.
    Orbit,
    /// Panning across the view.
    Pan,
    /// Looking around and flying.
    Fly,
    /// Dragging a handle of the gizmo.
    Gizmo,
    /// A press that becomes a click if the pointer stays put.
    Click {
        /// Where the press happened.
        origin: Point,
    },
}

/// The editor's view onto the preview: it owns the camera, the tool and the
/// drag, reads a [`ViewportInput`] each frame and edits the open document's
/// selection and transforms, and draws the grid, outlines, component gizmos and
/// handles over the 3D frame.
///
/// Each frame the shell calls [`Preview::sync`], then [`Viewport::update`], then
/// builds the 3D frame with [`Viewport::camera`] and draws
/// [`Viewport::draw`] over it into the same rectangle.
pub struct Viewport {
    /// The camera.
    camera: EditorCamera,
    /// The tool, its handles and the drag.
    gizmo: TransformGizmo,
    /// The colours of everything drawn.
    palette: ViewportPalette,
    /// What the pointer is doing.
    mode: Mode,
    /// The pointer position of the previous frame.
    last_pointer: Option<Point>,
    /// The rectangle of the previous frame.
    rect: Rect,
    /// The document the camera was last framed on.
    framed_for: Option<PathBuf>,
    /// Whether the ground grid is drawn.
    show_grid: bool,
}

impl Default for Viewport {
    /// A viewport with the default camera, the move tool and the grid on.
    fn default() -> Self {
        Self {
            camera: EditorCamera::default(),
            gizmo: TransformGizmo::default(),
            palette: ViewportPalette::default(),
            mode: Mode::Idle,
            last_pointer: None,
            rect: Rect::default(),
            framed_for: None,
            show_grid: true,
        }
    }
}

impl Viewport {
    /// A viewport with the default camera, the move tool and the grid on.
    pub fn new() -> Self {
        Self::default()
    }

    /// The camera the 3D frame is seen through.
    pub fn camera(&self) -> Camera {
        self.camera.camera()
    }

    /// The editor camera, to move it.
    pub fn editor_camera_mut(&mut self) -> &mut EditorCamera {
        &mut self.camera
    }

    /// The tool in force.
    pub fn tool(&self) -> Tool {
        self.gizmo.tool()
    }

    /// Chooses the tool.
    pub fn set_tool(&mut self, tool: Tool) {
        self.gizmo.set_tool(tool);
    }

    /// The orientation of the handles.
    pub fn space(&self) -> Space {
        self.gizmo.space()
    }

    /// Chooses the orientation of the handles.
    pub fn set_space(&mut self, space: Space) {
        self.gizmo.set_space(space);
    }

    /// Whether the ground grid is drawn.
    pub fn grid_visible(&self) -> bool {
        self.show_grid
    }

    /// Shows or hides the ground grid.
    pub fn set_grid_visible(&mut self, visible: bool) {
        self.show_grid = visible;
    }

    /// Replaces the colours of everything drawn.
    pub fn set_palette(&mut self, palette: ViewportPalette) {
        self.palette = palette;
    }

    /// The aspect ratio of the last rectangle, one before there was any.
    fn aspect(&self) -> f32 {
        if self.rect.size.height > 0.0 {
            self.rect.size.width / self.rect.size.height
        } else {
            1.0
        }
    }

    /// The projector of the camera into the last rectangle.
    fn projector(&self) -> Projector {
        Projector::new(self.camera.camera(), self.rect)
    }

    /// Frames the camera on the nodes of ordinary size in the preview, leaving out
    /// a ground plane or backdrop far larger than the rest.
    pub fn frame_scene(&mut self, preview: &Preview) {
        if let Some(bounds) = preview.framing_bounds() {
            self.camera.frame(&bounds, self.aspect());
        }
    }

    /// Frames the camera on the selected nodes, or on everything when none is
    /// selected or none has bounds.
    pub fn frame_selection(&mut self, document: Option<&OpenDocument>, preview: &Preview) {
        let selected: Option<Aabb> =
            document.and_then(|open| preview.bounds_of(open.selection().nodes()));
        match selected {
            Some(bounds) => self.camera.frame(&bounds, self.aspect()),
            None => self.frame_scene(preview),
        }
    }

    /// Frames a newly shown document once, when the preview has something to
    /// frame.
    fn frame_new_document(&mut self, document: Option<&OpenDocument>, preview: &Preview) {
        let Some(open) = document else {
            return;
        };
        let path = open.document().path();
        if self.framed_for.as_deref() == Some(path) || preview.scene_bounds().is_none() {
            return;
        }
        self.framed_for = Some(path.to_path_buf());
        self.frame_scene(preview);
    }

    /// Reads one frame of input: navigates the camera, picks, hovers and drags
    /// the gizmo, editing the open document's selection and transforms. A gizmo
    /// drag is one gesture, committed on release and cancelled by Escape, so a
    /// whole drag is one undo entry.
    pub fn update(
        &mut self,
        input: ViewportInput,
        mut document: Option<&mut OpenDocument>,
        preview: &Preview,
        _registry: &SubsystemRegistry,
    ) -> ViewportOutput {
        let mut output = ViewportOutput::idle(self.gizmo.tool());
        self.rect = input.rect;
        self.frame_new_document(document.as_deref(), preview);
        let delta = match (input.pointer, self.last_pointer) {
            (Some(now), Some(before)) => (now.x - before.x, now.y - before.y),
            _ => (0.0, 0.0),
        };
        self.last_pointer = input.pointer;
        if document.is_none() && self.mode == Mode::Gizmo {
            self.gizmo.abandon();
            self.mode = Mode::Idle;
        }
        if self.mode == Mode::Idle {
            self.begin_mode(&input, document.as_deref_mut(), preview, &mut output);
        } else {
            self.continue_mode(&input, delta, document.as_deref_mut(), preview, &mut output);
        }
        if self.mode == Mode::Idle {
            self.idle_input(&input, document, preview, &mut output);
        }
        output.tool = self.gizmo.tool();
        output.captured |= self.mode != Mode::Idle || self.gizmo.active_handle().is_some();
        output.cursor = self.cursor();
        output
    }

    /// The cursor for what the pointer is doing.
    fn cursor(&self) -> CursorHint {
        match self.mode {
            Mode::Orbit => CursorHint::Orbit,
            Mode::Pan => CursorHint::Pan,
            Mode::Fly => CursorHint::Look,
            Mode::Gizmo => CursorHint::Handle,
            _ if self.gizmo.active_handle().is_some() => CursorHint::Handle,
            _ => CursorHint::Default,
        }
    }

    /// Starts navigating, dragging a handle or a click, from a press over the
    /// viewport.
    fn begin_mode(
        &mut self,
        input: &ViewportInput,
        document: Option<&mut OpenDocument>,
        preview: &Preview,
        output: &mut ViewportOutput,
    ) {
        if !input.hovered {
            self.gizmo.clear_hover();
            return;
        }
        let projector = self.projector();
        let frame = document
            .as_deref()
            .and_then(|open| self.gizmo.frame(open, preview, &projector));
        self.gizmo.update_hover(frame, &projector, input.pointer);
        let (Some(pointer), true) = (input.pointer, input.pressed.any()) else {
            return;
        };
        if input.pressed.middle {
            self.mode = if input.modifiers.shift {
                Mode::Pan
            } else {
                Mode::Orbit
            };
        } else if input.pressed.secondary {
            self.mode = Mode::Fly;
        } else if input.pressed.primary && input.modifiers.alt {
            self.mode = Mode::Orbit;
        } else if input.pressed.primary {
            self.mode = Mode::Click { origin: pointer };
            if let Some(open) = document {
                match self.gizmo.begin(open, preview, &projector, pointer) {
                    Ok(true) => self.mode = Mode::Gizmo,
                    Ok(false) => {}
                    Err(error) => output.messages.push(error.to_string()),
                }
            }
        }
    }

    /// Carries on the mode in progress and ends it when its button is up.
    fn continue_mode(
        &mut self,
        input: &ViewportInput,
        delta: (f32, f32),
        document: Option<&mut OpenDocument>,
        preview: &Preview,
        output: &mut ViewportOutput,
    ) {
        match self.mode {
            Mode::Orbit => {
                self.camera.orbit(delta.0, delta.1);
                if !(input.held.middle || input.held.primary) {
                    self.mode = Mode::Idle;
                }
            }
            Mode::Pan => {
                self.camera.pan(delta.0, delta.1, self.rect.size.height);
                if !input.held.middle {
                    self.mode = Mode::Idle;
                }
            }
            Mode::Fly => {
                self.camera.look(delta.0, delta.1);
                self.camera.fly(
                    fly_direction(&input.keys_held),
                    input.delta_time,
                    input.modifiers.shift,
                );
                if !input.held.secondary {
                    self.mode = Mode::Idle;
                }
            }
            Mode::Gizmo => self.continue_gizmo(input, document, output),
            Mode::Click { origin } => self.continue_click(input, origin, document, preview, output),
            Mode::Idle => {}
        }
    }

    /// Moves, commits or cancels the drag of a handle.
    fn continue_gizmo(
        &mut self,
        input: &ViewportInput,
        document: Option<&mut OpenDocument>,
        output: &mut ViewportOutput,
    ) {
        let Some(open) = document else {
            return;
        };
        let projector = self.projector();
        let result = if input.keys_pressed.contains(&ViewportKey::Escape) {
            self.mode = Mode::Idle;
            self.gizmo.cancel(open).map(|()| false)
        } else if !input.held.primary {
            self.mode = Mode::Idle;
            self.gizmo
                .end(open)
                .inspect(|changed| output.committed = *changed)
        } else if let Some(pointer) = input.pointer {
            self.gizmo
                .drag_to(open, &projector, pointer, input.modifiers.control)
                .inspect(|changed| output.edited |= *changed)
        } else {
            Ok(false)
        };
        match result {
            Ok(changed) => output.edited |= changed,
            Err(error) => {
                output.messages.push(error.to_string());
                self.mode = Mode::Idle;
                self.gizmo.abandon();
            }
        }
    }

    /// Turns a press and release without a drag into a pick.
    fn continue_click(
        &mut self,
        input: &ViewportInput,
        origin: Point,
        document: Option<&mut OpenDocument>,
        preview: &Preview,
        output: &mut ViewportOutput,
    ) {
        if input.held.primary {
            return;
        }
        self.mode = Mode::Idle;
        let (Some(open), Some(pointer)) = (document, input.pointer) else {
            return;
        };
        if (pointer.x - origin.x).hypot(pointer.y - origin.y) > CLICK_SLOP {
            return;
        }
        let hit = self
            .projector()
            .ray(pointer)
            .and_then(|ray| pick(preview.bounds(), &ray));
        output.selection_changed = apply_click(open.selection_mut(), hit, input.modifiers);
    }

    /// Handles what applies while nothing is held: the wheel and the keys.
    fn idle_input(
        &mut self,
        input: &ViewportInput,
        document: Option<&mut OpenDocument>,
        preview: &Preview,
        output: &mut ViewportOutput,
    ) {
        if !input.hovered {
            return;
        }
        if input.scroll != 0.0 {
            self.camera.zoom(input.scroll);
        }
        let plain = !(input.modifiers.control || input.modifiers.alt || input.modifiers.logo);
        for key in &input.keys_pressed {
            match (key, plain) {
                (ViewportKey::F, true) => self.frame_selection(document.as_deref(), preview),
                (ViewportKey::X, true) => self.gizmo.toggle_space(),
                (key, true) => {
                    if let Some(tool) = Tool::from_key(*key) {
                        self.gizmo.set_tool(tool);
                        output.tool = tool;
                    }
                }
                _ => {}
            }
        }
    }

    /// Draws the ground grid, the outlines and wire shapes of the selection
    /// and the transform handles into `rect`, which is the rectangle the 3D
    /// frame was drawn into. Nothing is drawn outside it.
    pub fn draw(
        &self,
        list: &mut DrawList,
        rect: Rect,
        document: Option<&OpenDocument>,
        preview: &Preview,
        registry: &SubsystemRegistry,
    ) {
        let projector = Projector::new(self.camera.camera(), rect);
        list.push_clip(rect);
        if self.show_grid {
            draw_grid(list, &projector, &self.palette, self.camera.pivot());
        }
        if let Some(open) = document {
            draw_selection_outlines(
                list,
                &projector,
                preview,
                open.selection().nodes(),
                open.selection().primary(),
                &self.palette,
            );
            draw_component_gizmos(list, &projector, open, preview, registry, &self.palette);
            if let Some(frame) = self.gizmo.frame(open, preview, &projector) {
                draw_transform_gizmo(list, &projector, &self.gizmo, &frame, &self.palette);
            }
        }
        list.pop_clip();
    }
}

/// The direction the held keys fly the camera in: x right, y up, z forward.
fn fly_direction(held: &[ViewportKey]) -> Vec3 {
    let axis = |positive: ViewportKey, negative: ViewportKey| {
        f32::from(u8::from(held.contains(&positive)))
            - f32::from(u8::from(held.contains(&negative)))
    };
    Vec3::new(
        axis(ViewportKey::D, ViewportKey::A),
        axis(ViewportKey::E, ViewportKey::Q),
        axis(ViewportKey::W, ViewportKey::S),
    )
}
