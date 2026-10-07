//! A camera that orbits a target in response to drags and scrolls.

use fr_math::Vec3;

use crate::camera::Camera;

/// Orbits a camera around a target at a distance, turned by `yaw` around the vertical axis
/// and raised by `pitch` above the horizon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitController {
    /// The point the camera looks at.
    pub target: Vec3,
    /// The angle around the vertical axis, in radians.
    pub yaw: f32,
    /// The angle above the horizon, in radians.
    pub pitch: f32,
    /// The distance from the camera to the target.
    pub distance: f32,
    /// Radians of orbit per unit dragged.
    pub orbit_speed: f32,
    /// How far one unit of scrolling changes the distance, as a fraction of it.
    pub zoom_speed: f32,
    /// The lowest pitch in radians.
    pub min_pitch: f32,
    /// The highest pitch in radians.
    pub max_pitch: f32,
    /// The shortest distance zooming allows.
    pub min_distance: f32,
    /// The longest distance zooming allows.
    pub max_distance: f32,
}

impl OrbitController {
    /// Turns the camera by a drag of `dx` and `dy`.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * self.orbit_speed;
        self.pitch = (self.pitch + dy * self.orbit_speed).clamp(self.min_pitch, self.max_pitch);
    }

    /// Moves the camera closer or further by a scroll of `scroll`, positive meaning closer.
    pub fn zoom(&mut self, scroll: f32) {
        self.distance = (self.distance * (-scroll * self.zoom_speed).exp())
            .clamp(self.min_distance, self.max_distance);
    }

    /// Where the camera sits.
    pub fn position(&self) -> Vec3 {
        let offset = Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        );
        self.target + offset * self.distance
    }

    /// Puts `camera` at the orbit position, looking at the target.
    pub fn apply(&self, camera: &mut Camera) {
        camera.position = self.position();
        camera.target = self.target;
    }
}

impl Default for OrbitController {
    /// An orbit around the origin from five units away, pitching between plus and minus 1.5
    /// radians and zooming between a tenth of a unit and a thousand.
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            distance: 5.0,
            orbit_speed: 0.005,
            zoom_speed: 0.002,
            min_pitch: -1.5,
            max_pitch: 1.5,
            min_distance: 0.1,
            max_distance: 1000.0,
        }
    }
}
