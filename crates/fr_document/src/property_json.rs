//! The JSON form of property values, which carry their own type so that a
//! component whose schema is unavailable still round-trips.

use fr_math::{Quat, Vec3};
use fr_transform::Transform;
use serde_json::Value;

use crate::error::DocumentError;
use crate::json::{
    as_array, as_f32, as_floats, as_guid, as_i64, as_object, as_str, guid_or_none, guid_value,
    items_or_empty, number, object, required, string_or_empty,
};
use crate::property::{
    AssetReference, Color, ComponentReference, EntityReference, PropertyType, PropertyValue,
};

/// Writes floats as a JSON array.
fn floats(values: &[f32]) -> Value {
    Value::Array(values.iter().map(|value| number(*value)).collect())
}

/// Writes a transform as translation, rotation and scale.
pub fn write_transform(transform: &Transform) -> Value {
    let mut map = object();
    map.insert(
        "translation".to_owned(),
        floats(&transform.translation.to_array()),
    );
    map.insert(
        "rotation".to_owned(),
        floats(&transform.rotation.to_array()),
    );
    map.insert("scale".to_owned(), floats(&transform.scale.to_array()));
    Value::Object(map)
}

/// Reads a transform; an absent value is the identity and absent parts keep
/// their identity values.
///
/// # Errors
///
/// When a part is not the right shape.
pub fn read_transform(value: Option<&Value>, context: &str) -> Result<Transform, DocumentError> {
    let Some(value) = value else {
        return Ok(Transform::IDENTITY);
    };
    let map = as_object(value, context)?;
    let mut transform = Transform::IDENTITY;
    if let Some(part) = map.get("translation") {
        transform.translation = Vec3::from_array(as_floats::<3>(part, context)?);
    }
    if let Some(part) = map.get("rotation") {
        transform.rotation = Quat::from_array(as_floats::<4>(part, context)?);
    }
    if let Some(part) = map.get("scale") {
        transform.scale = Vec3::from_array(as_floats::<3>(part, context)?);
    }
    Ok(transform)
}

/// Writes an asset reference.
pub fn write_asset_reference(reference: &AssetReference) -> Value {
    let mut map = object();
    map.insert("asset".to_owned(), guid_value(reference.asset));
    map.insert(
        "last_known_path".to_owned(),
        Value::String(reference.last_known_path.clone()),
    );
    Value::Object(map)
}

/// Reads an asset reference; an absent value is unset.
///
/// # Errors
///
/// When a part is not the right shape.
pub fn read_asset_reference(
    value: Option<&Value>,
    context: &str,
) -> Result<AssetReference, DocumentError> {
    let Some(value) = value else {
        return Ok(AssetReference::default());
    };
    let map = as_object(value, context)?;
    Ok(AssetReference {
        asset: guid_or_none(map, "asset", context)?,
        last_known_path: string_or_empty(map, "last_known_path", context)?,
    })
}

/// Writes the fields of an entity reference into an object.
fn write_entity_fields(map: &mut serde_json::Map<String, Value>, reference: &EntityReference) {
    map.insert(
        "instances".to_owned(),
        Value::Array(
            reference
                .instances
                .iter()
                .map(|id| guid_value(*id))
                .collect(),
        ),
    );
    map.insert("entity".to_owned(), guid_value(reference.entity));
}

/// Reads the fields of an entity reference from an object.
fn read_entity_fields(
    map: &serde_json::Map<String, Value>,
    context: &str,
) -> Result<EntityReference, DocumentError> {
    let mut instances = Vec::new();
    for item in items_or_empty(map, "instances", context)? {
        instances.push(as_guid(item, context)?);
    }
    Ok(EntityReference {
        instances,
        entity: guid_or_none(map, "entity", context)?,
    })
}

/// The `value` member of a property value.
fn write_payload(value: &PropertyValue) -> Value {
    match value {
        PropertyValue::Bool(value) => Value::Bool(*value),
        PropertyValue::Int(value) => Value::from(*value),
        PropertyValue::Float(value) => number(*value),
        PropertyValue::String(value) | PropertyValue::Enum(value) => Value::String(value.clone()),
        PropertyValue::Vec3(value) => floats(&value.to_array()),
        PropertyValue::Color(value) => floats(&[value.r, value.g, value.b, value.a]),
        PropertyValue::Rotation(value) => floats(&value.to_array()),
        PropertyValue::Transform(value) => write_transform(value),
        PropertyValue::Asset(value) => write_asset_reference(value),
        PropertyValue::Entity(value) => {
            let mut map = object();
            write_entity_fields(&mut map, value);
            Value::Object(map)
        }
        PropertyValue::Component(value) => {
            let mut map = object();
            write_entity_fields(&mut map, &value.owner);
            map.insert("component".to_owned(), guid_value(value.component));
            Value::Object(map)
        }
        PropertyValue::Array(items) => Value::Array(items.iter().map(write_typed).collect()),
    }
}

/// Writes a value with its type: `{"type": …, "value": …}`.
pub fn write_typed(value: &PropertyValue) -> Value {
    let mut map = object();
    map.insert(
        "type".to_owned(),
        Value::String(value.kind().key().to_owned()),
    );
    map.insert("value".to_owned(), write_payload(value));
    Value::Object(map)
}

/// Reads the payload of a value of a known type.
fn read_payload(
    kind: PropertyType,
    value: &Value,
    context: &str,
) -> Result<PropertyValue, DocumentError> {
    Ok(match kind {
        PropertyType::Bool => PropertyValue::Bool(crate::json::as_bool(value, context)?),
        PropertyType::Int => PropertyValue::Int(as_i64(value, context)?),
        PropertyType::Float => PropertyValue::Float(as_f32(value, context)?),
        PropertyType::String => PropertyValue::String(as_str(value, context)?.to_owned()),
        PropertyType::Enum => PropertyValue::Enum(as_str(value, context)?.to_owned()),
        PropertyType::Vec3 => {
            PropertyValue::Vec3(Vec3::from_array(as_floats::<3>(value, context)?))
        }
        PropertyType::Color => {
            let [r, g, b, a] = as_floats::<4>(value, context)?;
            PropertyValue::Color(Color { r, g, b, a })
        }
        PropertyType::Rotation => {
            PropertyValue::Rotation(Quat::from_array(as_floats::<4>(value, context)?))
        }
        PropertyType::Transform => PropertyValue::Transform(read_transform(Some(value), context)?),
        PropertyType::Asset => PropertyValue::Asset(read_asset_reference(Some(value), context)?),
        PropertyType::Entity => {
            PropertyValue::Entity(read_entity_fields(as_object(value, context)?, context)?)
        }
        PropertyType::Component => {
            let map = as_object(value, context)?;
            PropertyValue::Component(ComponentReference {
                owner: read_entity_fields(map, context)?,
                component: guid_or_none(map, "component", context)?,
            })
        }
        PropertyType::Array => {
            let mut items = Vec::new();
            for item in as_array(value, context)? {
                items.push(read_typed(item, context)?);
            }
            PropertyValue::Array(items)
        }
    })
}

/// Reads a value with its type from an object holding `type` and `value`.
///
/// # Errors
///
/// When the type is unknown or the value does not fit it.
pub fn read_typed(source: &Value, context: &str) -> Result<PropertyValue, DocumentError> {
    let map = as_object(source, context)?;
    let key = as_str(required(map, "type", context)?, context)?;
    let kind = PropertyType::from_key(key).ok_or_else(|| {
        DocumentError::malformed(format!("{context}: unknown property type \"{key}\""))
    })?;
    read_payload(kind, required(map, "value", context)?, context)
}
