//! The game schema: the component types a game registered, exported as JSON so
//! that a tool can know them without loading game code.

use serde_json::Value;

use crate::error::DocumentError;
use crate::format::check_format;
use crate::json::{
    self, as_f32, as_object, as_str, bool_or, int_or, items_or_empty, number, object, required,
    string_or_empty,
};
use crate::property::PropertyType;
use crate::property_json::{read_typed, write_typed};
use crate::schema::{ComponentSchema, ComponentSource, EnumOption, PropertySchema, SchemaSet};

/// The format name of a game schema file.
const GAME_SCHEMA_FORMAT: &str = "forge.game_schema";

/// The version every game schema file is written at and read at.
pub const GAME_SCHEMA_FORMAT_VERSION: u32 = 1;

/// The component types of a game with the fingerprint of the registry they were
/// exported from.
#[derive(Clone, Debug, PartialEq)]
pub struct GameSchema {
    /// The fingerprint of every registered type, engine and game.
    pub fingerprint: String,
    /// The game's own types.
    pub types: Vec<ComponentSchema>,
}

impl GameSchema {
    /// The game's types in a set, with the fingerprint of the whole set.
    pub fn from_set(set: &SchemaSet) -> Self {
        Self {
            fingerprint: set.fingerprint(),
            types: set
                .types()
                .iter()
                .filter(|schema| schema.source == ComponentSource::Game)
                .cloned()
                .collect(),
        }
    }

    /// Adds the game's types to a set, replacing the types it had from a game.
    pub fn apply_to(&self, set: &mut SchemaSet) {
        set.remove_source(ComponentSource::Game);
        for schema in &self.types {
            set.add(schema.clone());
        }
    }
}

/// Writes a property schema.
fn write_property(property: &PropertySchema) -> Value {
    let mut map = object();
    map.insert("key".to_owned(), Value::String(property.key.clone()));
    map.insert("label".to_owned(), Value::String(property.label.clone()));
    if let Value::Object(typed) = write_typed(&property.default_value) {
        map.extend(typed);
    }
    map.insert(
        "options".to_owned(),
        Value::Array(
            property
                .options
                .iter()
                .map(|option| {
                    let mut entry = object();
                    entry.insert("key".to_owned(), Value::String(option.key.clone()));
                    entry.insert("label".to_owned(), Value::String(option.label.clone()));
                    Value::Object(entry)
                })
                .collect(),
        ),
    );
    map.insert(
        "element_type".to_owned(),
        Value::String(property.element_type.key().to_owned()),
    );
    map.insert(
        "max_items".to_owned(),
        Value::from(i64::from(property.max_items)),
    );
    map.insert("minimum".to_owned(), number(property.minimum));
    map.insert("maximum".to_owned(), number(property.maximum));
    map.insert("step".to_owned(), number(property.step));
    Value::Object(map)
}

/// Writes the type keys of a list.
fn write_keys(keys: &[String]) -> Value {
    Value::Array(keys.iter().cloned().map(Value::String).collect())
}

/// Writes a component schema.
fn write_schema(schema: &ComponentSchema) -> Value {
    let mut map = object();
    map.insert("key".to_owned(), Value::String(schema.key.clone()));
    map.insert("label".to_owned(), Value::String(schema.label.clone()));
    map.insert(
        "category".to_owned(),
        Value::String(schema.category.clone()),
    );
    map.insert(
        "schema_version".to_owned(),
        Value::from(i64::from(schema.schema_version)),
    );
    map.insert(
        "allow_multiple".to_owned(),
        Value::Bool(schema.allow_multiple),
    );
    map.insert("removable".to_owned(), Value::Bool(schema.removable));
    map.insert(
        "required_types".to_owned(),
        write_keys(&schema.required_types),
    );
    map.insert(
        "conflicting_types".to_owned(),
        write_keys(&schema.conflicting_types),
    );
    map.insert(
        "properties".to_owned(),
        Value::Array(schema.properties.iter().map(write_property).collect()),
    );
    Value::Object(map)
}

/// Serialises a game schema.
pub fn write_game_schema(schema: &GameSchema) -> String {
    let mut map = object();
    map.insert(
        "format".to_owned(),
        Value::String(GAME_SCHEMA_FORMAT.to_owned()),
    );
    map.insert(
        "format_version".to_owned(),
        Value::from(i64::from(GAME_SCHEMA_FORMAT_VERSION)),
    );
    map.insert(
        "fingerprint".to_owned(),
        Value::String(schema.fingerprint.clone()),
    );
    map.insert(
        "types".to_owned(),
        Value::Array(schema.types.iter().map(write_schema).collect()),
    );
    json::write(&Value::Object(map))
}

/// Reads a property schema.
fn read_property(source: &Value, context: &str) -> Result<PropertySchema, DocumentError> {
    let map = as_object(source, context)?;
    let key = as_str(required(map, "key", context)?, context)?.to_owned();
    let context = format!("{context}.{key}");
    let default_value = read_typed(source, &context)?;
    let mut options = Vec::new();
    for option in items_or_empty(map, "options", &context)? {
        let entry = as_object(option, &context)?;
        options.push(EnumOption {
            key: string_or_empty(entry, "key", &context)?,
            label: string_or_empty(entry, "label", &context)?,
        });
    }
    let element = string_or_empty(map, "element_type", &context)?;
    let float = |name: &str, default: f32| -> Result<f32, DocumentError> {
        map.get(name)
            .map_or(Ok(default), |value| as_f32(value, &context))
    };
    Ok(PropertySchema {
        label: string_or_empty(map, "label", &context)?,
        kind: default_value.kind(),
        default_value,
        options,
        element_type: PropertyType::from_key(&element).unwrap_or(PropertyType::Float),
        max_items: u32::try_from(int_or(map, "max_items", 0, &context)?).unwrap_or(0),
        minimum: float("minimum", 0.0)?,
        maximum: float("maximum", 0.0)?,
        step: float("step", 0.01)?,
        key,
    })
}

/// Reads the type keys of a list field.
fn read_keys(
    map: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<Vec<String>, DocumentError> {
    items_or_empty(map, key, context)?
        .iter()
        .map(|item| as_str(item, context).map(str::to_owned))
        .collect()
}

/// Reads a component schema of a game type.
fn read_schema(source: &Value) -> Result<ComponentSchema, DocumentError> {
    let map = as_object(source, "type")?;
    let key = as_str(required(map, "key", "type")?, "type.key")?.to_owned();
    let context = format!("type {key}");
    let mut properties = Vec::new();
    for property in items_or_empty(map, "properties", &context)? {
        properties.push(read_property(property, &context)?);
    }
    Ok(ComponentSchema {
        label: string_or_empty(map, "label", &context)?,
        category: string_or_empty(map, "category", &context)?,
        schema_version: u32::try_from(int_or(map, "schema_version", 1, &context)?)
            .map_err(|_| DocumentError::malformed(format!("{context}: invalid schema_version")))?,
        source: ComponentSource::Game,
        allow_multiple: bool_or(map, "allow_multiple", false, &context)?,
        removable: bool_or(map, "removable", true, &context)?,
        intrinsic: false,
        required_types: read_keys(map, "required_types", &context)?,
        conflicting_types: read_keys(map, "conflicting_types", &context)?,
        properties,
        key,
    })
}

/// Parses a game schema.
///
/// # Errors
///
/// When the text is not a game schema of the current version.
pub fn read_game_schema(source: &str) -> Result<GameSchema, DocumentError> {
    let value = json::parse(source)?;
    let map = as_object(&value, "game schema")?;
    check_format(map, GAME_SCHEMA_FORMAT, GAME_SCHEMA_FORMAT_VERSION)?;
    let mut types = Vec::new();
    for entry in items_or_empty(map, "types", "game schema")? {
        types.push(read_schema(entry)?);
    }
    Ok(GameSchema {
        fingerprint: string_or_empty(map, "fingerprint", "game schema")?,
        types,
    })
}
