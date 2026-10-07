//! A free-flying camera turned by mouse motion and moved along its own axes.

use fr_math::Vec3;

use crate::camera::Camera;

/// The highest pitch in radians, just short of straight up or down.
const MAX_PITCH: f32 = 1.55;

/// A camera that looks where `yaw` and `pitch` point and flies along its own axes.
///
/// A yaw of zero looks along negative Z; a positive yaw turns the view to the left, and a
/// positive pitch looks up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyController {
    /// Where the camera is.
    pub position: Vec3,
    /// The angle around the vertical axis, in radians.
    pub yaw: f32,
    /// The angle above the horizon, in radians.
    pub pitch: f32,
    /// Radians of turn per unit of mouse motion.
    pub look_speed: f32,
    /// The flying speed in units per second.
    pub speed: f32,
    /// The lowest speed scrolling allows.
    pub min_speed: f32,
    /// The highest speed scrolling allows.
    pub max_speed: f32,
    /// How much faster the camera flies while boosting.
    pub boost: f32,
    /// How far one unit of scrolling changes the speed, as a fraction of it.
    pub scroll_speed: f32,
}

impl FlyController {
    /// A controller at `position` looking at `target`.
    pub fn looking_at(position: Vec3, target: Vec3) -> Self {
        let direction = (target - position).normalize_or(Vec3::NEG_Z);
        Self {
            yaw: (-direction.x).atan2(-direction.z),
            pitch: direction.y.clamp(-1.0, 1.0).asin(),
            ..Self::at(position)
        }
    }

    /// A controller at `position` looking along negative Z.
    pub fn at(position: Vec3) -> Self {
        Self {
            position,
            yaw: 0.0,
            pitch: 0.0,
            look_speed: 0.0025,
            speed: 4.0,
            min_speed: 0.1,
            max_speed: 200.0,
            boost: 4.0,
            scroll_speed: 0.005,
        }
    }

    /// The direction the camera looks in.
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            -self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    /// The direction to the camera's right, level with the horizon.
    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    /// Turns the camera by a mouse motion of `dx` to the right and `dy` downwards.
    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * self.look_speed;
        self.pitch = (self.pitch - dy * self.look_speed).clamp(-MAX_PITCH, MAX_PITCH);
    }

    /// Flies for `seconds`: `direction` holds the wish along the right, world up and forward
    /// axes, each from minus one to one, and `boosting` multiplies the speed.
    pub fn fly(&mut self, direction: Vec3, boosting: bool, seconds: f32) {
        let wish =
            self.right() * direction.x + Vec3::Y * direction.y + self.forward() * direction.z;
        let factor = if boosting { self.boost } else { 1.0 };
        self.position += wish.clamp_length_max(1.0) * (self.speed * factor * seconds);
    }

    /// Changes the flying speed by a scroll of `scroll`, positive meaning faster.
    pub fn scroll(&mut self, scroll: f32) {
        self.speed =
            (self.speed * (scroll * self.scroll_speed).exp()).clamp(self.min_speed, self.max_speed);
    }

    /// Puts `camera` at the controller's position, looking along its direction.
    pub fn apply(&self, camera: &mut Camera) {
        camera.position = self.position;
        camera.target = self.position + self.forward();
    }
}

impl Default for FlyController {
    /// A controller at the origin looking along negative Z.
    fn default() -> Self {
        Self::at(Vec3::ZERO)
    }
}
