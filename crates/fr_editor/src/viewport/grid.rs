//! The ground grid, drawn as lines over the scene.

use fr_color::Rgba;
use fr_math::Vec3;
use fr_render::DrawList;

use super::palette::ViewportPalette;
use super::projector::Projector;

/// How many grid steps the grid reaches from the focus in each direction.
const HALF_LINES: i32 = 24;

/// Every how many lines a major line is drawn.
const MAJOR_EVERY: i32 = 10;

/// How many pieces each line is cut into so that it can fade out toward its
/// ends.
const FADE_PIECES: i32 = 8;

/// The width of a grid line in logical pixels.
const LINE_WIDTH: f32 = 1.0;

/// The opacity of the lines through the origin.
const AXIS_ALPHA: f32 = 0.55;

/// The distance to the focus, in grid steps, the spacing aims at.
const STEPS_PER_DISTANCE: f32 = 0.2;

/// The spacing of the grid for a camera at `distance` from its focus: a power
/// of ten, so lines stay readable from a prop to a level.
fn grid_step(distance: f32) -> f32 {
    10.0_f32.powf((distance * STEPS_PER_DISTANCE).max(1e-3).log10().floor())
}

/// Draws the ground plane's grid around `focus`, with the X and Z axes through
/// the origin in their axis colours. The overlay has no depth test, so the grid
/// is drawn faintly and fades out with distance from the focus.
pub fn draw_grid(
    list: &mut DrawList,
    projector: &Projector,
    palette: &ViewportPalette,
    focus: Vec3,
) {
    let step = grid_step((projector.camera().position - focus).length());
    let reach = step * HALF_LINES as f32;
    let center_x = (focus.x / step).round() * step;
    let center_z = (focus.z / step).round() * step;
    for index in -HALF_LINES..=HALF_LINES {
        let offset = index as f32 * step;
        let major = (index + (center_x / step).round() as i32).rem_euclid(MAJOR_EVERY) == 0;
        let color = if major {
            palette.grid_major
        } else {
            palette.grid_minor
        };
        let along_z_x = center_x + offset;
        let along_x_z = center_z + offset;
        fading_line(
            list,
            projector,
            Vec3::new(along_z_x, 0.0, center_z - reach),
            Vec3::new(along_z_x, 0.0, center_z + reach),
            (focus, reach),
            color,
        );
        fading_line(
            list,
            projector,
            Vec3::new(center_x - reach, 0.0, along_x_z),
            Vec3::new(center_x + reach, 0.0, along_x_z),
            (focus, reach),
            color,
        );
    }
    let x_axis = palette.axis_x.alpha(AXIS_ALPHA);
    let z_axis = palette.axis_z.alpha(AXIS_ALPHA);
    fading_line(
        list,
        projector,
        Vec3::new(-reach + center_x, 0.0, 0.0),
        Vec3::new(reach + center_x, 0.0, 0.0),
        (focus, reach),
        x_axis,
    );
    fading_line(
        list,
        projector,
        Vec3::new(0.0, 0.0, -reach + center_z),
        Vec3::new(0.0, 0.0, reach + center_z),
        (focus, reach),
        z_axis,
    );
}

/// Draws a line in pieces whose opacity falls with their distance from `fade.0`,
/// reaching zero at `fade.1`.
fn fading_line(
    list: &mut DrawList,
    projector: &Projector,
    from: Vec3,
    to: Vec3,
    fade: (Vec3, f32),
    color: Rgba,
) {
    for piece in 0..FADE_PIECES {
        let start = from.lerp(to, piece as f32 / FADE_PIECES as f32);
        let end = from.lerp(to, (piece + 1) as f32 / FADE_PIECES as f32);
        let middle = (start + end) * 0.5;
        let falloff = 1.0 - ((middle - fade.0) * Vec3::new(1.0, 0.0, 1.0)).length() / fade.1;
        if falloff > 0.0 {
            projector.draw_line(list, start, end, LINE_WIDTH, color.alpha(falloff));
        }
    }
}
