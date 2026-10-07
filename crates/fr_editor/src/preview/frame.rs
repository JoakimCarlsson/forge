//! The light an editor frame is lit with when the document has none.

use fr_camera::Camera;
use fr_light::DirectionalLight;
use fr_math::Vec3;

/// The illuminance of the headlight, a little under a bright day.
const HEADLIGHT_INTENSITY: f32 = 2.5;

/// How far the headlight is turned to the right of the view direction, as a
/// fraction of it.
const HEADLIGHT_SIDE: f32 = 0.25;

/// How far the headlight is turned down from the view direction, as a fraction
/// of it.
const HEADLIGHT_DOWN: f32 = 0.45;

/// A directional light shining from behind the camera over its shoulder, so
/// whatever the camera sees is lit and the viewport is never black. It casts no
/// shadows.
pub fn headlight(camera: &Camera) -> DirectionalLight {
    let forward = (camera.target - camera.position)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    let right = forward.cross(camera.up).try_normalize().unwrap_or(Vec3::X);
    let up = right.cross(forward);
    let direction = forward + right * HEADLIGHT_SIDE - up * HEADLIGHT_DOWN;
    DirectionalLight {
        direction: direction.normalize_or_zero(),
        color: Vec3::ONE,
        intensity: HEADLIGHT_INTENSITY,
        cast_shadows: false,
        shadow_distance: 0.0,
    }
}
