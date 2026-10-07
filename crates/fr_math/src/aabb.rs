//! Axis aligned bounding boxes.

use glam::Vec3;

/// An axis aligned box from its min to its max corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aabb {
    /// The corner with the smallest coordinates.
    pub min: Vec3,
    /// The corner with the largest coordinates.
    pub max: Vec3,
}

impl Aabb {
    /// A box from its corners.
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// The box of centre `center` and half sizes `extents`.
    pub fn from_center_extents(center: Vec3, extents: Vec3) -> Self {
        Self::new(center - extents, center + extents)
    }

    /// The centre of the box.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// The half sizes of the box.
    pub fn extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// Whether this box and `other` share any volume or touch.
    pub fn overlaps(&self, other: &Self) -> bool {
        !(other.min.x > self.max.x
            || other.min.y > self.max.y
            || other.min.z > self.max.z
            || self.min.x > other.max.x
            || self.min.y > other.max.y
            || self.min.z > other.max.z)
    }

    /// Whether `other` lies completely inside this box.
    pub fn contains(&self, other: &Self) -> bool {
        self.min.x <= other.min.x
            && self.min.y <= other.min.y
            && self.min.z <= other.min.z
            && other.max.x <= self.max.x
            && other.max.y <= self.max.y
            && other.max.z <= self.max.z
    }

    /// The smallest box that holds both boxes.
    pub fn union(&self, other: &Self) -> Self {
        Self::new(self.min.min(other.min), self.max.max(other.max))
    }

    /// The box grown by `margin` on every side.
    pub fn inflate(&self, margin: f32) -> Self {
        let r = Vec3::splat(margin);
        Self::new(self.min - r, self.max + r)
    }

    /// The area of the surface of the box.
    pub fn surface_area(&self) -> f32 {
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// The fraction along the segment `origin + t * translation` at which it enters the box, when
    /// it reaches the box before `max_fraction`.
    pub fn ray_entry(&self, origin: Vec3, translation: Vec3, max_fraction: f32) -> Option<f32> {
        let origins = origin.to_array();
        let translations = translation.to_array();
        let min = self.min.to_array();
        let max = self.max.to_array();
        let mut entry = 0.0_f32;
        let mut exit = max_fraction;
        for axis in 0..3 {
            if translations[axis].abs() <= 1e-8 {
                if origins[axis] < min[axis] || origins[axis] > max[axis] {
                    return None;
                }
                continue;
            }
            let inverse = 1.0 / translations[axis];
            let mut first = (min[axis] - origins[axis]) * inverse;
            let mut last = (max[axis] - origins[axis]) * inverse;
            if first > last {
                std::mem::swap(&mut first, &mut last);
            }
            entry = entry.max(first);
            exit = exit.min(last);
            if entry > exit {
                return None;
            }
        }
        Some(entry)
    }
}
