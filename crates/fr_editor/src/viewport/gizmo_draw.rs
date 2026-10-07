//! Drawing the selection, the component gizmos and the transform handles over
//! the scene with the draw list.

use fr_authoring::OpenDocument;
use fr_color::Rgba;
use fr_document::Guid;
use fr_math::{Point, Rect, Size, Vec3};
use fr_render::{DrawList, Quad};

use super::handles::{Axis, GizmoFrame, Handle, HandleSet, handle_segments, plane_corners};
use super::palette::ViewportPalette;
use super::projector::Projector;
use super::transform_gizmo::TransformGizmo;
use crate::preview::{NodeBounds, Preview};
use crate::subsystems::{GizmoSink, SubsystemRegistry};

/// The width of a handle's line in logical pixels.
const HANDLE_WIDTH: f32 = 3.0;

/// The width of a wire shape in logical pixels.
const OUTLINE_WIDTH: f32 = 1.5;

/// The width of the outline of a selected node in logical pixels.
const SELECTION_WIDTH: f32 = 2.0;

/// The side of the tip of an arrow or scale handle in logical pixels.
const TIP_SIZE: f32 = 9.0;

/// The side of the box at the pivot that scales uniformly, in logical pixels.
const UNIFORM_SIZE: f32 = 12.0;

/// The opacity of the half of a ring that faces away from the camera.
const BACK_ALPHA: f32 = 0.3;

/// The opacity of a plane handle's outline.
const PLANE_ALPHA: f32 = 0.85;

/// The twelve edges of a box as pairs of corner indices, corners numbered as
/// [`crate::preview::OrientedBox::corners`] does.
const BOX_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

/// The colour of an axis.
fn axis_color(palette: &ViewportPalette, axis: Axis) -> Rgba {
    match axis {
        Axis::X => palette.axis_x,
        Axis::Y => palette.axis_y,
        Axis::Z => palette.axis_z,
    }
}

/// A square of `size` logical pixels centred on a point.
fn centered_square(center: Point, size: f32) -> Rect {
    Rect::new(
        Point::new(center.x - size * 0.5, center.y - size * 0.5),
        Size::new(size, size),
    )
}

/// Draws the outline of every selected node as a wire box: the oriented box
/// when the node has one part, otherwise the box around all of them. The
/// primary node is drawn brighter.
pub fn draw_selection_outlines(
    list: &mut DrawList,
    projector: &Projector,
    preview: &Preview,
    nodes: &[Guid],
    primary: Option<Guid>,
    palette: &ViewportPalette,
) {
    for node in nodes {
        let Some(bounds) = preview.node_bounds(*node) else {
            continue;
        };
        let color = if primary == Some(*node) {
            palette.primary
        } else {
            palette.selection
        };
        draw_node_outline(list, projector, bounds, color);
    }
}

/// Draws the wire box of one node's bounds.
fn draw_node_outline(list: &mut DrawList, projector: &Projector, bounds: &NodeBounds, color: Rgba) {
    let corners = bounds.outline();
    for (from, to) in BOX_EDGES {
        projector.draw_line(list, corners[from], corners[to], SELECTION_WIDTH, color);
    }
}

/// Draws the wire shapes the subsystems contribute for the components of the
/// selected entities, at their world transforms.
pub fn draw_component_gizmos(
    list: &mut DrawList,
    projector: &Projector,
    document: &OpenDocument,
    preview: &Preview,
    registry: &SubsystemRegistry,
    palette: &ViewportPalette,
) {
    let content = document.document().content();
    let schemas = document.document().schemas();
    for node in document.selection().nodes() {
        let (Some(entity), Some(world)) =
            (content.find_entity(*node), preview.world_transform(*node))
        else {
            continue;
        };
        for component in entity
            .components
            .iter()
            .filter(|component| component.enabled)
        {
            let mut sink = GizmoSink::new(palette.gizmo);
            registry.gizmo(component, schemas, &world, &mut sink);
            for line in sink.lines() {
                projector.draw_line(list, line.from, line.to, OUTLINE_WIDTH, line.color);
            }
        }
    }
}

/// Draws the handles of the tool in force on a frame, the one under the pointer
/// or being dragged in the hover colour.
pub fn draw_transform_gizmo(
    list: &mut DrawList,
    projector: &Projector,
    gizmo: &TransformGizmo,
    frame: &GizmoFrame,
    palette: &ViewportPalette,
) {
    let Some(set) = gizmo.tool().handle_set() else {
        return;
    };
    let active = gizmo.active_handle();
    let camera_position = projector.camera().position;
    for handle in set.handles() {
        let color = handle_color(handle, active == Some(handle), palette);
        match handle {
            Handle::Axis(axis) => {
                draw_axis_handle(list, projector, set, frame, axis, color, camera_position);
            }
            Handle::Plane(axis) => draw_plane_handle(list, projector, frame, axis, color),
            Handle::Ring(_) => {
                for segment in handle_segments(set, handle, frame, camera_position) {
                    let tone = if segment.front {
                        color
                    } else {
                        color.alpha(BACK_ALPHA)
                    };
                    projector.draw_line(list, segment.from, segment.to, HANDLE_WIDTH, tone);
                }
            }
            Handle::Uniform => {
                if let Some(center) = projector.project(frame.pivot) {
                    list.quad(Quad::filled(centered_square(center, UNIFORM_SIZE), color));
                }
            }
        }
    }
}

/// The colour of a handle: its axis colour, or the hover colour when active.
fn handle_color(handle: Handle, active: bool, palette: &ViewportPalette) -> Rgba {
    if active {
        return palette.hover;
    }
    match handle {
        Handle::Axis(axis) | Handle::Plane(axis) | Handle::Ring(axis) => axis_color(palette, axis),
        Handle::Uniform => palette.neutral,
    }
}

/// Draws an arrow or scale handle: its shaft and a tip.
fn draw_axis_handle(
    list: &mut DrawList,
    projector: &Projector,
    set: HandleSet,
    frame: &GizmoFrame,
    axis: Axis,
    color: Rgba,
    camera_position: Vec3,
) {
    for segment in handle_segments(set, Handle::Axis(axis), frame, camera_position) {
        projector.draw_line(list, segment.from, segment.to, HANDLE_WIDTH, color);
        if let Some(tip) = projector.project(segment.to) {
            let quad = Quad::filled(centered_square(tip, TIP_SIZE), color);
            let quad = if set == HandleSet::Move {
                quad.rotated(std::f32::consts::FRAC_PI_4)
            } else {
                quad
            };
            list.quad(quad);
        }
    }
}

/// Draws a plane handle as an outline with a dot at its middle.
fn draw_plane_handle(
    list: &mut DrawList,
    projector: &Projector,
    frame: &GizmoFrame,
    axis: Axis,
    color: Rgba,
) {
    let corners = plane_corners(frame, axis);
    for pair in corners.windows(2) {
        projector.draw_line(
            list,
            pair[0],
            pair[1],
            OUTLINE_WIDTH,
            color.alpha(PLANE_ALPHA),
        );
    }
    let middle = (corners[0] + corners[2]) * 0.5;
    if let Some(center) = projector.project(middle) {
        list.quad(Quad::filled(centered_square(center, 4.0), color));
    }
}
