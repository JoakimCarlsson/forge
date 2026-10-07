//! View and projection matrices for a right-handed, Y-up world and a GPU
//! whose clip-space depth runs from zero to one.

use glam::{Mat4, Vec3};

/// The matrix taking world space into the view space of a camera at `eye` looking at `target`.
pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    glam::camera::rh::view::look_at_mat4(eye, target, up)
}

/// The perspective matrix for a vertical field of view of `fov_y` radians and
/// a viewport of `aspect` width over height, between `near` and `far`.
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    glam::camera::rh::proj::directx::perspective(fov_y, aspect, near, far)
}

/// The orthographic matrix for the box between `left` and `right`, `bottom`
/// and `top`, and `near` and `far` along the view direction.
pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    glam::camera::rh::proj::directx::orthographic(left, right, bottom, top, near, far)
}
