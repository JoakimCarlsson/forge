//! The camera the author looks through: an orbit around a pivot that can also
//! fly, pan, zoom to the pivot and frame what is selected.

use fr_camera::{Camera, OrbitController, PerspectiveProjection, Projection};
use fr_math::{Aabb, Vec3};

/// Radians of rotation per logical pixel of drag.
const ROTATE_SPEED: f32 = 0.005;

/// The fraction of the distance one wheel notch zooms by, as an exponent.
const ZOOM_PER_NOTCH: f32 = 0.12;

/// The distance the camera starts at from its pivot.
const INITIAL_DISTANCE: f32 = 10.0;

/// The yaw the camera starts at, in radians.
const INITIAL_YAW: f32 = 0.7;

/// The pitch the camera starts at, in radians.
const INITIAL_PITCH: f32 = 0.45;

/// The vertical field of view, in radians.
const FIELD_OF_VIEW: f32 = 1.0;

/// The speed of flying in world units per second.
const FLY_SPEED: f32 = 8.0;

/// What holding shift multiplies the fly speed by.
const FLY_BOOST: f32 = 4.0;

/// How much of the view a framed box leaves around it, as a factor on the
/// distance.
const FRAME_MARGIN: f32 = 1.2;

/// The smallest radius a framed box is treated as having.
const MINIMUM_FRAME_RADIUS: f32 = 0.5;

/// A perspective camera that orbits a pivot, kept as an orbit controller so
/// zooming always moves toward the pivot.
#[derive(Clone, Copy, Debug)]
pub struct EditorCamera {
    /// The pivot, yaw, pitch and distance.
    orbit: OrbitController,
    /// The vertical field of view in radians.
    fov_y: f32,
}

impl Default for EditorCamera {
    /// A camera ten units from the origin looking down at it from a corner.
    fn default() -> Self {
        Self {
            orbit: OrbitController {
                target: Vec3::ZERO,
                yaw: INITIAL_YAW,
                pitch: INITIAL_PITCH,
                distance: INITIAL_DISTANCE,
                orbit_speed: ROTATE_SPEED,
                zoom_speed: ZOOM_PER_NOTCH,
                min_pitch: -1.55,
                max_pitch: 1.55,
                min_distance: 0.02,
                max_distance: 50_000.0,
            },
            fov_y: FIELD_OF_VIEW,
        }
    }
}

impl EditorCamera {
    /// The point the camera orbits and zooms toward.
    pub fn pivot(&self) -> Vec3 {
        self.orbit.target
    }

    /// The distance from the camera to its pivot.
    pub fn distance(&self) -> f32 {
        self.orbit.distance
    }

    /// The camera as the renderer takes it. The clip planes follow the
    /// distance so both a small prop and a whole level stay free of
    /// z-fighting.
    pub fn camera(&self) -> Camera {
        let near = (self.orbit.distance * 0.02).clamp(0.02, 5.0);
        let far = (self.orbit.distance * 100.0).clamp(1000.0, 200_000.0);
        let mut camera = Camera {
            projection: Projection::Perspective(PerspectiveProjection {
                fov_y: self.fov_y,
                near,
                far,
            }),
            ..Camera::default()
        };
        self.orbit.apply(&mut camera);
        camera
    }

    /// The unit vectors of the camera: right, up and forward.
    fn axes(&self) -> (Vec3, Vec3, Vec3) {
        let forward = (self.orbit.target - self.orbit.position())
            .try_normalize()
            .unwrap_or(Vec3::NEG_Z);
        let right = forward.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
        (right, right.cross(forward), forward)
    }

    /// Turns the camera around its pivot by a drag of `dx` and `dy` pixels.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.orbit.orbit(dx, dy);
    }

    /// Turns the camera around its own position by a drag of `dx` and `dy`
    /// pixels, as looking around does; the pivot goes with it.
    pub fn look(&mut self, dx: f32, dy: f32) {
        let eye = self.orbit.position();
        self.orbit.orbit(dx, dy);
        let offset = self.orbit.position() - self.orbit.target;
        self.orbit.target = eye - offset;
    }

    /// Slides the camera and its pivot across the view by a drag of `dx` and
    /// `dy` pixels in a viewport `height` pixels tall, so the scene follows the
    /// pointer.
    pub fn pan(&mut self, dx: f32, dy: f32, height: f32) {
        let per_pixel = 2.0 * self.orbit.distance * (self.fov_y * 0.5).tan() / height.max(1.0);
        let (right, up, _) = self.axes();
        self.orbit.target += (up * dy - right * dx) * per_pixel;
    }

    /// Moves toward or away from the pivot by wheel notches, positive closer.
    pub fn zoom(&mut self, notches: f32) {
        self.orbit.zoom(notches);
    }

    /// Flies for `seconds` along `direction`, whose x is right, y is up and z
    /// is forward in the camera's own axes; the pivot goes with the camera.
    pub fn fly(&mut self, direction: Vec3, seconds: f32, boost: bool) {
        let speed = FLY_SPEED * if boost { FLY_BOOST } else { 1.0 };
        let (right, up, forward) = self.axes();
        let world = right * direction.x + up * direction.y + forward * direction.z;
        self.orbit.target += world.normalize_or_zero() * speed * seconds;
    }

    /// Places the camera so a box fills the view, keeping its direction. The
    /// view's shorter side decides the distance, so the box fits whatever the
    /// shape of the viewport.
    pub fn frame(&mut self, bounds: &Aabb, aspect: f32) {
        let radius = bounds.extents().length().max(MINIMUM_FRAME_RADIUS);
        let vertical = self.fov_y * 0.5;
        let horizontal = (vertical.tan() * aspect.max(0.1)).atan();
        self.orbit.target = bounds.center();
        self.orbit.distance = (radius / vertical.min(horizontal).sin() * FRAME_MARGIN)
            .clamp(self.orbit.min_distance, self.orbit.max_distance);
    }
}
