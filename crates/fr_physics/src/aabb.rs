//! Axis aligned bounding boxes.

use fr_core::Vec3;

/// An axis aligned box from its lower to its upper corner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aabb {
    /// The corner with the smallest coordinates.
    pub lower: Vec3,
    /// The corner with the largest coordinates.
    pub upper: Vec3,
}

impl Aabb {
    /// A box from its corners.
    pub const fn new(lower: Vec3, upper: Vec3) -> Self {
        Self { lower, upper }
    }

    /// The box of centre `center` and half sizes `extents`.
    pub fn from_center_extents(center: Vec3, extents: Vec3) -> Self {
        Self::new(center - extents, center + extents)
    }

    /// The centre of the box.
    pub fn center(&self) -> Vec3 {
        (self.lower + self.upper) * 0.5
    }

    /// The half sizes of the box.
    pub fn extents(&self) -> Vec3 {
        (self.upper - self.lower) * 0.5
    }

    /// Whether this box and `other` share any volume or touch.
    pub fn overlaps(&self, other: &Self) -> bool {
        !(other.lower.x > self.upper.x
            || other.lower.y > self.upper.y
            || other.lower.z > self.upper.z
            || self.lower.x > other.upper.x
            || self.lower.y > other.upper.y
            || self.lower.z > other.upper.z)
    }

    /// Whether `other` lies completely inside this box.
    pub fn contains(&self, other: &Self) -> bool {
        self.lower.x <= other.lower.x
            && self.lower.y <= other.lower.y
            && self.lower.z <= other.lower.z
            && other.upper.x <= self.upper.x
            && other.upper.y <= self.upper.y
            && other.upper.z <= self.upper.z
    }

    /// The smallest box that holds both boxes.
    pub fn union(&self, other: &Self) -> Self {
        Self::new(self.lower.min(other.lower), self.upper.max(other.upper))
    }

    /// The box grown by `margin` on every side.
    pub fn inflate(&self, margin: f32) -> Self {
        let r = Vec3::splat(margin);
        Self::new(self.lower - r, self.upper + r)
    }

    /// The area of the surface of the box.
    pub fn surface_area(&self) -> f32 {
        let d = self.upper - self.lower;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// The fraction along the segment `origin + t * translation` at which it enters the box, when
    /// it reaches the box before `max_fraction`.
    pub fn ray_entry(&self, origin: Vec3, translation: Vec3, max_fraction: f32) -> Option<f32> {
        let origins = origin.to_array();
        let translations = translation.to_array();
        let lower = self.lower.to_array();
        let upper = self.upper.to_array();
        let mut entry = 0.0_f32;
        let mut exit = max_fraction;
        for axis in 0..3 {
            if translations[axis].abs() <= 1e-8 {
                if origins[axis] < lower[axis] || origins[axis] > upper[axis] {
                    return None;
                }
                continue;
            }
            let inverse = 1.0 / translations[axis];
            let mut first = (lower[axis] - origins[axis]) * inverse;
            let mut last = (upper[axis] - origins[axis]) * inverse;
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
