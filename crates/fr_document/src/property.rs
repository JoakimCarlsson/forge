//! Typed property values of component records.

use fr_math::{Quat, Vec3};
use fr_transform::Transform;

use crate::guid::Guid;

/// A colour in linear channels with an alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Opacity, one being opaque.
    pub a: f32,
}

impl Color {
    /// An opaque colour from its channels.
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// A colour from its channels and opacity.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// The colour as a vector of its three colour channels.
    pub const fn to_vec3(self) -> Vec3 {
        Vec3::new(self.r, self.g, self.b)
    }
}

impl Default for Color {
    /// Opaque white.
    fn default() -> Self {
        Self::rgb(1.0, 1.0, 1.0)
    }
}

/// The type of a property value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyType {
    /// A boolean.
    Bool,
    /// A 64-bit integer.
    Int,
    /// A 32-bit float.
    Float,
    /// Free text.
    String,
    /// The key of one option of a fixed set.
    Enum,
    /// Three floats.
    Vec3,
    /// A colour with an alpha.
    Color,
    /// A quaternion.
    Rotation,
    /// A translation, rotation and scale.
    Transform,
    /// A reference to an asset.
    Asset,
    /// A reference to an entity.
    Entity,
    /// A reference to a component of an entity.
    Component,
    /// A list of values.
    Array,
}

/// Every property type with its serialised key.
const TYPE_KEYS: [(PropertyType, &str); 13] = [
    (PropertyType::Bool, "bool"),
    (PropertyType::Int, "int"),
    (PropertyType::Float, "float"),
    (PropertyType::String, "string"),
    (PropertyType::Enum, "enum"),
    (PropertyType::Vec3, "vec3"),
    (PropertyType::Color, "color"),
    (PropertyType::Rotation, "rotation"),
    (PropertyType::Transform, "transform"),
    (PropertyType::Asset, "asset"),
    (PropertyType::Entity, "entity"),
    (PropertyType::Component, "component"),
    (PropertyType::Array, "array"),
];

impl PropertyType {
    /// The serialised key of the type.
    pub fn key(self) -> &'static str {
        TYPE_KEYS
            .iter()
            .find(|(candidate, _)| *candidate == self)
            .map_or("bool", |(_, key)| key)
    }

    /// The type a serialised key names.
    pub fn from_key(key: &str) -> Option<Self> {
        TYPE_KEYS
            .iter()
            .find(|(_, candidate)| *candidate == key)
            .map(|(property, _)| *property)
    }
}

/// A reference to an asset by identity, with the path it last had.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssetReference {
    /// The asset identity.
    pub asset: Guid,
    /// The path the asset last had, for display and recovery.
    pub last_known_path: String,
}

/// A reference to an entity, through the prefab instances that contain it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityReference {
    /// The chain of prefab instances leading to the entity, outermost first.
    pub instances: Vec<Guid>,
    /// The entity inside the innermost prefab or scene.
    pub entity: Guid,
}

/// A reference to one component of an entity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentReference {
    /// The entity that owns the component.
    pub owner: EntityReference,
    /// The component.
    pub component: Guid,
}

/// A property value.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    /// A boolean.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f32),
    /// Free text.
    String(String),
    /// The key of an option.
    Enum(String),
    /// A vector.
    Vec3(Vec3),
    /// A colour.
    Color(Color),
    /// A rotation.
    Rotation(Quat),
    /// A transform.
    Transform(Transform),
    /// An asset reference.
    Asset(AssetReference),
    /// An entity reference.
    Entity(EntityReference),
    /// A component reference.
    Component(ComponentReference),
    /// A list of values.
    Array(Vec<PropertyValue>),
}

impl PropertyValue {
    /// The type of the value.
    pub fn kind(&self) -> PropertyType {
        match self {
            Self::Bool(_) => PropertyType::Bool,
            Self::Int(_) => PropertyType::Int,
            Self::Float(_) => PropertyType::Float,
            Self::String(_) => PropertyType::String,
            Self::Enum(_) => PropertyType::Enum,
            Self::Vec3(_) => PropertyType::Vec3,
            Self::Color(_) => PropertyType::Color,
            Self::Rotation(_) => PropertyType::Rotation,
            Self::Transform(_) => PropertyType::Transform,
            Self::Asset(_) => PropertyType::Asset,
            Self::Entity(_) => PropertyType::Entity,
            Self::Component(_) => PropertyType::Component,
            Self::Array(_) => PropertyType::Array,
        }
    }

    /// The default value of a type: false, zero, empty, the identity or unset.
    pub fn of_kind(kind: PropertyType) -> Self {
        match kind {
            PropertyType::Bool => Self::Bool(false),
            PropertyType::Int => Self::Int(0),
            PropertyType::Float => Self::Float(0.0),
            PropertyType::String => Self::String(String::new()),
            PropertyType::Enum => Self::Enum(String::new()),
            PropertyType::Vec3 => Self::Vec3(Vec3::ZERO),
            PropertyType::Color => Self::Color(Color::default()),
            PropertyType::Rotation => Self::Rotation(Quat::IDENTITY),
            PropertyType::Transform => Self::Transform(Transform::IDENTITY),
            PropertyType::Asset => Self::Asset(AssetReference::default()),
            PropertyType::Entity => Self::Entity(EntityReference::default()),
            PropertyType::Component => Self::Component(ComponentReference::default()),
            PropertyType::Array => Self::Array(Vec::new()),
        }
    }

    /// The boolean, when the value is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The integer, when the value is one.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// The float, when the value is a float or an integer.
    pub fn as_float(&self) -> Option<f32> {
        match self {
            Self::Float(value) => Some(*value),
            Self::Int(value) => Some(*value as f32),
            _ => None,
        }
    }

    /// The text, when the value is a string or an enum option.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::String(value) | Self::Enum(value) => Some(value),
            _ => None,
        }
    }

    /// The vector, when the value is one.
    pub fn as_vec3(&self) -> Option<Vec3> {
        match self {
            Self::Vec3(value) => Some(*value),
            _ => None,
        }
    }

    /// The colour, when the value is one.
    pub fn as_color(&self) -> Option<Color> {
        match self {
            Self::Color(value) => Some(*value),
            _ => None,
        }
    }

    /// The rotation, when the value is one.
    pub fn as_rotation(&self) -> Option<Quat> {
        match self {
            Self::Rotation(value) => Some(*value),
            _ => None,
        }
    }

    /// The asset reference, when the value is one.
    pub fn as_asset(&self) -> Option<&AssetReference> {
        match self {
            Self::Asset(value) => Some(value),
            _ => None,
        }
    }
}

/// A named property of a component record.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyEntry {
    /// The property key.
    pub key: String,
    /// The property value.
    pub value: PropertyValue,
}
