//! The rows of the scene settings: clear colour, ambient light and gravity,
//! shown when nothing is selected in a scene.

use std::rc::Rc;

use fr_authoring::{Command, OpenDocument};
use fr_document::{Color, SceneSettings};
use fr_math::Vec3;

use crate::subsystems::{InspectorRow, RowValue, no_change};

/// The largest ambient intensity the row accepts.
const MAX_AMBIENT: f32 = 10.0;

/// The most physics sub-steps the row accepts.
const MAX_SUB_STEPS: f32 = 32.0;

/// A row of the settings: `update` returns the settings with the edited value
/// stored, or none when the value does not fit.
fn settings_row(
    base: SceneSettings,
    (key, label): (&str, &str),
    value: RowValue,
    range: Option<(f32, f32)>,
    update: fn(SceneSettings, &RowValue) -> Option<SceneSettings>,
) -> InspectorRow {
    InspectorRow {
        key: key.to_owned(),
        label: label.to_owned(),
        value,
        range,
        step: Some(0.05),
        read_only: false,
        apply: Rc::new(move |edited| match update(base, &edited) {
            Some(settings) => Command::SetSceneSettings(settings),
            None => no_change(),
        }),
    }
}

/// An opaque colour of a vector's channels.
fn colour_of(vector: Vec3) -> Color {
    Color::rgb(vector.x, vector.y, vector.z)
}

/// The rows of a scene's settings.
pub fn scene_rows(document: &OpenDocument) -> Vec<InspectorRow> {
    let base = *document.document().settings();
    vec![
        settings_row(
            base,
            ("clear_color", "Clear Colour"),
            RowValue::Color(base.clear_color),
            None,
            |mut settings, value| match value {
                RowValue::Color(color) => {
                    settings.clear_color = *color;
                    Some(settings)
                }
                _ => None,
            },
        ),
        settings_row(
            base,
            ("sky_color", "Ambient Sky"),
            RowValue::Color(colour_of(base.ambient.sky_color)),
            None,
            |mut settings, value| match value {
                RowValue::Color(color) => {
                    settings.ambient.sky_color = color.to_vec3();
                    Some(settings)
                }
                _ => None,
            },
        ),
        settings_row(
            base,
            ("ground_color", "Ambient Ground"),
            RowValue::Color(colour_of(base.ambient.ground_color)),
            None,
            |mut settings, value| match value {
                RowValue::Color(color) => {
                    settings.ambient.ground_color = color.to_vec3();
                    Some(settings)
                }
                _ => None,
            },
        ),
        settings_row(
            base,
            ("ambient_intensity", "Ambient Intensity"),
            RowValue::Float(base.ambient.intensity),
            Some((0.0, MAX_AMBIENT)),
            |mut settings, value| match value {
                RowValue::Float(intensity) => {
                    settings.ambient.intensity = *intensity;
                    Some(settings)
                }
                _ => None,
            },
        ),
        settings_row(
            base,
            ("gravity", "Gravity"),
            RowValue::Vec3(base.physics.gravity),
            None,
            |mut settings, value| match value {
                RowValue::Vec3(gravity) => {
                    settings.physics.gravity = *gravity;
                    Some(settings)
                }
                _ => None,
            },
        ),
        settings_row(
            base,
            ("sub_steps", "Sub-steps"),
            RowValue::Int(i64::from(base.physics.sub_steps)),
            Some((1.0, MAX_SUB_STEPS)),
            |mut settings, value| match value {
                RowValue::Int(steps) => {
                    settings.physics.sub_steps = (*steps).clamp(1, MAX_SUB_STEPS as i64) as u32;
                    Some(settings)
                }
                _ => None,
            },
        ),
    ]
}
