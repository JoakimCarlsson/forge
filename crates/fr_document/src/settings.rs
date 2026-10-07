//! The settings a scene carries beside its entities: the background, the
//! ambient light and the physics.

use fr_math::Vec3;
use serde_json::Value;

use crate::error::DocumentError;
use crate::json::{as_f32, as_floats, as_object, int_or, number, object};
use crate::property::Color;

/// The sky and ground light added everywhere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbientSettings {
    /// The linear colour of the light from straight above.
    pub sky_color: Vec3,
    /// The linear colour of the light from straight below.
    pub ground_color: Vec3,
    /// How bright the light is.
    pub intensity: f32,
}

impl Default for AmbientSettings {
    /// A faint blue sky over a dim warm ground.
    fn default() -> Self {
        Self {
            sky_color: Vec3::new(0.55, 0.7, 1.0),
            ground_color: Vec3::new(0.35, 0.3, 0.25),
            intensity: 0.3,
        }
    }
}

/// The physics world of a scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsSettings {
    /// The acceleration of gravity in metres per second squared.
    pub gravity: Vec3,
    /// The sub-steps of every fixed step.
    pub sub_steps: u32,
}

impl Default for PhysicsSettings {
    /// Gravity of ten metres per second squared downwards and four sub-steps.
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -10.0, 0.0),
            sub_steps: 4,
        }
    }
}

/// The settings of a scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneSettings {
    /// The colour the frame is cleared to before anything is drawn, as sRGB like every
    /// colour of the user interface; the colours of components are linear.
    pub clear_color: Color,
    /// The ambient light.
    pub ambient: AmbientSettings,
    /// The physics world.
    pub physics: PhysicsSettings,
}

impl Default for SceneSettings {
    /// A pale sky blue background with the default ambient light and physics.
    fn default() -> Self {
        Self {
            clear_color: Color::rgb(0.45, 0.6, 0.8),
            ambient: AmbientSettings::default(),
            physics: PhysicsSettings::default(),
        }
    }
}

/// Writes floats as a JSON array.
fn floats(values: &[f32]) -> Value {
    Value::Array(values.iter().map(|value| number(*value)).collect())
}

/// Writes scene settings.
pub fn write_scene_settings(settings: &SceneSettings) -> Value {
    let mut ambient = object();
    ambient.insert(
        "sky_color".to_owned(),
        floats(&settings.ambient.sky_color.to_array()),
    );
    ambient.insert(
        "ground_color".to_owned(),
        floats(&settings.ambient.ground_color.to_array()),
    );
    ambient.insert("intensity".to_owned(), number(settings.ambient.intensity));
    let mut physics = object();
    physics.insert(
        "gravity".to_owned(),
        floats(&settings.physics.gravity.to_array()),
    );
    physics.insert(
        "sub_steps".to_owned(),
        Value::from(i64::from(settings.physics.sub_steps)),
    );
    let color = settings.clear_color;
    let mut map = object();
    map.insert(
        "clear_color".to_owned(),
        floats(&[color.r, color.g, color.b, color.a]),
    );
    map.insert("ambient".to_owned(), Value::Object(ambient));
    map.insert("physics".to_owned(), Value::Object(physics));
    Value::Object(map)
}

/// Reads scene settings; absent parts keep their defaults.
///
/// # Errors
///
/// When a part is present and not the right shape.
pub fn read_scene_settings(source: &Value) -> Result<SceneSettings, DocumentError> {
    let map = as_object(source, "settings")?;
    let mut settings = SceneSettings::default();
    if let Some(value) = map.get("clear_color") {
        let [r, g, b, a] = as_floats::<4>(value, "settings.clear_color")?;
        settings.clear_color = Color { r, g, b, a };
    }
    if let Some(value) = map.get("ambient") {
        let ambient = as_object(value, "settings.ambient")?;
        if let Some(part) = ambient.get("sky_color") {
            settings.ambient.sky_color =
                Vec3::from_array(as_floats::<3>(part, "settings.ambient.sky_color")?);
        }
        if let Some(part) = ambient.get("ground_color") {
            settings.ambient.ground_color =
                Vec3::from_array(as_floats::<3>(part, "settings.ambient.ground_color")?);
        }
        if let Some(part) = ambient.get("intensity") {
            settings.ambient.intensity = as_f32(part, "settings.ambient.intensity")?;
        }
    }
    if let Some(value) = map.get("physics") {
        let physics = as_object(value, "settings.physics")?;
        if let Some(part) = physics.get("gravity") {
            settings.physics.gravity =
                Vec3::from_array(as_floats::<3>(part, "settings.physics.gravity")?);
        }
        let sub_steps = int_or(
            physics,
            "sub_steps",
            i64::from(settings.physics.sub_steps),
            "settings.physics",
        )?;
        settings.physics.sub_steps = u32::try_from(sub_steps)
            .ok()
            .filter(|steps| *steps > 0)
            .ok_or_else(|| {
                DocumentError::malformed("settings.physics.sub_steps: expected a positive integer")
            })?;
    }
    Ok(settings)
}
