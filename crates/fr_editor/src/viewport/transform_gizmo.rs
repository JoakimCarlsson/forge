//! The transform gizmo: tools, handles and the drag that edits the selection.
//!
//! A drag runs as one gesture on the open document: it begins on press, previews
//! a `SetTransform` per selected node on every move, and commits on release as a
//! single undo entry or is cancelled to restore where it began.

use fr_authoring::{AuthoringError, Command, OpenDocument};
use fr_document::{EntityDocument, Guid};
use fr_math::{Point, Quat, Vec3};
use fr_transform::Transform;

use super::handles::{
    Anchor, Delta, GIZMO_PIXELS, GizmoFrame, Handle, HandleSet, Snap, anchor_for, hit_test,
    solve_drag,
};
use super::input::ViewportKey;
use super::projector::Projector;
use crate::preview::Preview;

/// The tool in force: what a drag on the gizmo does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tool {
    /// No handles; clicks select.
    Select,
    /// Handles that translate.
    Move,
    /// Rings that rotate.
    Rotate,
    /// Handles that scale.
    Scale,
}

impl Tool {
    /// The tool a key chooses: Q, W, E and R.
    pub fn from_key(key: ViewportKey) -> Option<Self> {
        match key {
            ViewportKey::Q => Some(Self::Select),
            ViewportKey::W => Some(Self::Move),
            ViewportKey::E => Some(Self::Rotate),
            ViewportKey::R => Some(Self::Scale),
            _ => None,
        }
    }

    /// The name of the tool.
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Move => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
        }
    }

    /// The handles the tool shows; none for the select tool.
    pub fn handle_set(self) -> Option<HandleSet> {
        match self {
            Self::Select => None,
            Self::Move => Some(HandleSet::Move),
            Self::Rotate => Some(HandleSet::Rotate),
            Self::Scale => Some(HandleSet::Scale),
        }
    }
}

/// Whether the handles follow the primary node's rotation or the world's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Space {
    /// The handles are turned with the primary selected node.
    Local,
    /// The handles are aligned with the world axes.
    World,
}

/// One node a drag moves, with where it began.
#[derive(Clone, Copy, Debug)]
struct Target {
    /// The node.
    node: Guid,
    /// Its transform relative to its parent when the drag began.
    start_local: Transform,
    /// Its transform in the world when the drag began.
    start_world: Transform,
    /// The world transform of its parent.
    parent: Transform,
}

/// A drag in progress.
#[derive(Clone, Debug)]
struct Drag {
    /// The handle held.
    handle: Handle,
    /// What the pointer is measured against.
    anchor: Anchor,
    /// The gizmo's frame when the drag began.
    frame: GizmoFrame,
    /// The nodes being edited.
    targets: Vec<Target>,
    /// The change the latest move produced.
    current: Option<Delta>,
}

/// The tool, the handle under the pointer and the drag in progress.
#[derive(Clone, Debug)]
pub struct TransformGizmo {
    /// The tool in force.
    tool: Tool,
    /// The orientation of the handles.
    space: Space,
    /// The handle under the pointer.
    hover: Option<Handle>,
    /// The drag in progress.
    drag: Option<Drag>,
}

impl Default for TransformGizmo {
    /// The move tool with world aligned handles.
    fn default() -> Self {
        Self {
            tool: Tool::Move,
            space: Space::World,
            hover: None,
            drag: None,
        }
    }
}

impl TransformGizmo {
    /// The tool in force.
    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Chooses the tool.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.hover = None;
    }

    /// The orientation of the handles.
    pub fn space(&self) -> Space {
        self.space
    }

    /// Chooses the orientation of the handles.
    pub fn set_space(&mut self, space: Space) {
        self.space = space;
    }

    /// Switches between local and world handles.
    pub fn toggle_space(&mut self) {
        self.space = match self.space {
            Space::Local => Space::World,
            Space::World => Space::Local,
        };
    }

    /// The handle under the pointer, or being dragged.
    pub fn active_handle(&self) -> Option<Handle> {
        self.drag.as_ref().map(|drag| drag.handle).or(self.hover)
    }

    /// Whether a drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The frame of the gizmo on the primary selected node at a size that stays
    /// constant on screen; none when the tool has no handles, nothing is
    /// selected or the node is not in the preview. While a drag is in progress
    /// the frame follows the move.
    pub fn frame(
        &self,
        document: &OpenDocument,
        preview: &Preview,
        projector: &Projector,
    ) -> Option<GizmoFrame> {
        if let Some(drag) = &self.drag {
            return Some(match drag.current {
                Some(Delta::Translate(offset)) => GizmoFrame {
                    pivot: drag.frame.pivot + offset,
                    ..drag.frame
                },
                _ => drag.frame,
            });
        }
        self.tool.handle_set()?;
        let primary = document.selection().primary()?;
        let world = preview.world_transform(primary)?;
        let basis = if self.space == Space::Local || self.tool == Tool::Scale {
            world.rotation
        } else {
            Quat::IDENTITY
        };
        Some(GizmoFrame {
            pivot: world.translation,
            basis,
            length: projector.world_per_pixel(world.translation)? * GIZMO_PIXELS,
        })
    }

    /// Updates which handle the pointer is over.
    pub fn update_hover(
        &mut self,
        frame: Option<GizmoFrame>,
        projector: &Projector,
        pointer: Option<Point>,
    ) {
        self.hover = match (self.tool.handle_set(), frame, pointer) {
            (Some(set), Some(frame), Some(pointer)) => hit_test(set, &frame, projector, pointer),
            _ => None,
        };
    }

    /// Forgets the handle under the pointer.
    pub fn clear_hover(&mut self) {
        self.hover = None;
    }

    /// Begins a drag on the handle under the pointer as a gesture on the
    /// document. Returns whether a drag began, which it does not when no handle
    /// is under the pointer.
    ///
    /// # Errors
    ///
    /// The document's refusal to begin a gesture.
    pub fn begin(
        &mut self,
        document: &mut OpenDocument,
        preview: &Preview,
        projector: &Projector,
        pointer: Point,
    ) -> Result<bool, AuthoringError> {
        let (Some(set), Some(frame)) = (
            self.tool.handle_set(),
            self.frame(document, preview, projector),
        ) else {
            return Ok(false);
        };
        let Some(handle) = hit_test(set, &frame, projector, pointer) else {
            return Ok(false);
        };
        let Some(anchor) = anchor_for(handle, &frame, projector, pointer) else {
            return Ok(false);
        };
        let targets = drag_targets(document, preview);
        if targets.is_empty() {
            return Ok(false);
        }
        document.begin_gesture(self.tool.label())?;
        self.drag = Some(Drag {
            handle,
            anchor,
            frame,
            targets,
            current: None,
        });
        Ok(true)
    }

    /// Moves the drag to a pointer position, previewing the new transforms on
    /// the document. Returns whether the document changed.
    ///
    /// # Errors
    ///
    /// The document's refusal of a command.
    pub fn drag_to(
        &mut self,
        document: &mut OpenDocument,
        projector: &Projector,
        pointer: Point,
        snap: bool,
    ) -> Result<bool, AuthoringError> {
        let (Some(set), Some(drag)) = (self.tool.handle_set(), self.drag.as_mut()) else {
            return Ok(false);
        };
        let Some(delta) = solve_drag(
            drag.handle,
            set,
            drag.anchor,
            &drag.frame,
            projector,
            pointer,
            snap.then(Snap::default),
        ) else {
            return Ok(false);
        };
        drag.current = Some(delta);
        let commands: Vec<Command> = drag
            .targets
            .iter()
            .map(|target| Command::SetTransform {
                node: target.node,
                transform: transformed(target, delta, drag.frame.pivot),
            })
            .collect();
        let command = match <[Command; 1]>::try_from(commands) {
            Ok([single]) => single,
            Err(commands) => Command::Batch {
                label: self.tool.label().to_owned(),
                commands,
            },
        };
        Ok(document.preview(&command)?.changed)
    }

    /// Ends the drag, recording everything it changed as one undo entry.
    /// Returns whether anything changed.
    ///
    /// # Errors
    ///
    /// The document's failure to commit.
    pub fn end(&mut self, document: &mut OpenDocument) -> Result<bool, AuthoringError> {
        self.drag = None;
        document.commit_gesture()
    }

    /// Abandons the drag, restoring the document to where it began.
    ///
    /// # Errors
    ///
    /// The document's failure to cancel.
    pub fn cancel(&mut self, document: &mut OpenDocument) -> Result<(), AuthoringError> {
        self.drag = None;
        document.cancel_gesture()
    }

    /// Forgets a drag whose document is gone.
    pub fn abandon(&mut self) {
        self.drag = None;
    }
}

/// Whether a node has an ancestor in the list.
fn has_selected_ancestor(content: &EntityDocument, node: Guid, selected: &[Guid]) -> bool {
    let mut current = content.node_parent(node);
    while current.valid() {
        if selected.contains(&current) {
            return true;
        }
        current = content.node_parent(current);
    }
    false
}

/// The nodes a drag edits: the selected ones that are in the preview and do not
/// sit under another selected node, which follows its parent.
fn drag_targets(document: &OpenDocument, preview: &Preview) -> Vec<Target> {
    let content = document.document().content();
    let selected = document.selection().nodes();
    selected
        .iter()
        .filter(|node| !has_selected_ancestor(content, **node, selected))
        .filter_map(|node| {
            let start_local = content
                .find_entity(*node)
                .map(|entity| entity.transform)
                .or_else(|| content.find_instance(*node).map(|found| found.transform))?;
            let parent = content.node_parent(*node);
            Some(Target {
                node: *node,
                start_local,
                start_world: preview.world_transform(*node)?,
                parent: preview
                    .world_transform(parent)
                    .unwrap_or(Transform::IDENTITY),
            })
        })
        .collect()
}

/// The transform of a target, relative to its parent, after a change in the
/// world. Only the parts the change touches are recomputed, so a move leaves
/// rotation and scale exactly as they were.
fn transformed(target: &Target, delta: Delta, pivot: Vec3) -> Transform {
    let into_parent = target.parent.matrix().inverse();
    match delta {
        Delta::Translate(offset) => target.start_local.with_translation(
            into_parent.transform_point3(target.start_world.translation + offset),
        ),
        Delta::Rotate(rotation) => {
            let place = pivot + rotation * (target.start_world.translation - pivot);
            let turned = (rotation * target.start_world.rotation).normalize();
            Transform {
                translation: into_parent.transform_point3(place),
                rotation: (target.parent.rotation.inverse() * turned).normalize(),
                scale: target.start_local.scale,
            }
        }
        Delta::Scale(factor) => target
            .start_local
            .with_scale(target.start_local.scale * factor),
    }
}
