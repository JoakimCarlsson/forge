//! Translation, rotation and scale of an object in space.

use fr_math::{Mat3, Mat4, Quat, Vec3};

/// A translation, rotation and non-uniform scale, applied in scale, rotate,
/// translate order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Where the origin of the object sits.
    pub translation: Vec3,
    /// How the object is turned around its origin.
    pub rotation: Quat,
    /// How much the object is stretched along each of its own axes.
    pub scale: Vec3,
}

impl Transform {
    /// The transform that leaves everything where it is.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// A transform that only moves by `translation`.
    pub const fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    /// A transform that only turns by `rotation`.
    pub const fn from_rotation(rotation: Quat) -> Self {
        Self {
            rotation,
            ..Self::IDENTITY
        }
    }

    /// A transform that only stretches by `scale`.
    pub const fn from_scale(scale: Vec3) -> Self {
        Self {
            scale,
            ..Self::IDENTITY
        }
    }

    /// The same transform moved to `translation`.
    pub const fn with_translation(mut self, translation: Vec3) -> Self {
        self.translation = translation;
        self
    }

    /// The same transform turned to `rotation`.
    pub const fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    /// The same transform stretched to `scale`.
    pub const fn with_scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }

    /// A transform at `eye` turned so that its forward axis, negative Z, points at `target`
    /// with `up` as the direction that is up. A target at the eye or along `up` keeps the
    /// identity rotation.
    pub fn looking_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let forward = (target - eye).normalize_or_zero();
        let right = forward.cross(up).normalize_or_zero();
        if right == Vec3::ZERO {
            return Self::from_translation(eye);
        }
        let above = right.cross(forward);
        Self {
            translation: eye,
            rotation: Quat::from_mat3(&Mat3::from_cols(right, above, -forward)),
            scale: Vec3::ONE,
        }
    }

    /// Splits an affine `matrix` back into translation, rotation and scale.
    pub fn from_matrix(matrix: Mat4) -> Self {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// The matrix that takes points of the object into its parent's space.
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// This transform placed inside `parent`, so that the result takes points
    /// of the object into the parent's own parent space.
    pub fn then(&self, parent: &Self) -> Self {
        Self::from_matrix(parent.matrix() * self.matrix())
    }
}

impl Default for Transform {
    /// The identity transform.
    fn default() -> Self {
        Self::IDENTITY
    }
}
