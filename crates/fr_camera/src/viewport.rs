//! The rectangle of the window a camera draws into.

use fr_math::Vec2;

/// The size of the area a camera draws into, in the units pointer positions use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// The width and height.
    pub size: Vec2,
}

impl Viewport {
    /// A viewport of `width` by `height`.
    pub const fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vec2::new(width, height),
        }
    }

    /// The width over the height, or one when the height is zero.
    pub fn aspect(&self) -> f32 {
        if self.size.y <= 0.0 {
            return 1.0;
        }
        self.size.x / self.size.y
    }
}
