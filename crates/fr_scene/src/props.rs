//! Typed reads of a component record's properties, falling back to the
//! defaults of its schema.

use fr_document::{AssetReference, Color, ComponentRecord, ComponentSchema, PropertyValue};
use fr_math::Vec3;

/// A component record read through its schema: a property the record does not
/// carry, or carries with another type, reads as the schema's default.
pub struct Props<'a> {
    /// The record being read.
    record: &'a ComponentRecord,
    /// The schema whose defaults fill the gaps.
    schema: &'a ComponentSchema,
}

impl<'a> Props<'a> {
    /// Reads a record through its schema.
    pub fn new(record: &'a ComponentRecord, schema: &'a ComponentSchema) -> Self {
        Self { record, schema }
    }

    /// The value of a property: the record's when it has the schema's type,
    /// otherwise the schema's default.
    fn value(&self, key: &str) -> Option<&'a PropertyValue> {
        let property = self.schema.find_property(key)?;
        match self.record.property(key) {
            Some(value) if value.kind() == property.kind => Some(value),
            _ => Some(&property.default_value),
        }
    }

    /// A boolean property.
    pub fn boolean(&self, key: &str) -> bool {
        self.value(key)
            .and_then(PropertyValue::as_bool)
            .unwrap_or(false)
    }

    /// A float property.
    pub fn float(&self, key: &str) -> f32 {
        self.value(key)
            .and_then(PropertyValue::as_float)
            .unwrap_or(0.0)
    }

    /// An enum or text property.
    pub fn text(&self, key: &str) -> &'a str {
        self.value(key)
            .and_then(PropertyValue::as_text)
            .unwrap_or("")
    }

    /// A vector property.
    pub fn vec3(&self, key: &str) -> Vec3 {
        self.value(key)
            .and_then(PropertyValue::as_vec3)
            .unwrap_or(Vec3::ZERO)
    }

    /// A colour property.
    pub fn color(&self, key: &str) -> Color {
        self.value(key)
            .and_then(PropertyValue::as_color)
            .unwrap_or_default()
    }

    /// An asset reference property.
    pub fn asset(&self, key: &str) -> AssetReference {
        self.value(key)
            .and_then(PropertyValue::as_asset)
            .cloned()
            .unwrap_or_default()
    }
}
