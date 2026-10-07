//! The versioned JSON files of the authored data: `.scene` and `.prefab`.

use serde_json::{Map, Value};

use crate::error::DocumentError;
use crate::guid::Guid;
use crate::json::{
    self, as_i64, as_object, as_str, bool_or, guid_or_none, guid_value, int_or, items_or_empty,
    object, required, string_or_empty,
};
use crate::property::PropertyEntry;
use crate::property_json::{
    read_asset_reference, read_transform, read_typed, write_asset_reference, write_transform,
    write_typed,
};
use crate::records::{
    ComponentRecord, EntityDocument, EntityRecord, PrefabInstanceRecord, PropertyOverride,
};
use crate::settings::{SceneSettings, read_scene_settings, write_scene_settings};

/// The version every scene file is written at and read at.
pub const SCENE_FORMAT_VERSION: u32 = 1;

/// The version every prefab file is written at and read at.
pub const PREFAB_FORMAT_VERSION: u32 = 1;

/// Which of the two document formats a file uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatKind {
    /// A `.scene` file.
    Scene,
    /// A `.prefab` file.
    Prefab,
}

impl FormatKind {
    /// The format name stored in the file.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scene => "forge.scene",
            Self::Prefab => "forge.prefab",
        }
    }

    /// The version the format is read and written at.
    pub const fn version(self) -> u32 {
        match self {
            Self::Scene => SCENE_FORMAT_VERSION,
            Self::Prefab => PREFAB_FORMAT_VERSION,
        }
    }
}

/// A scene file: its identity, entities and settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneAsset {
    /// The asset identity from the sidecar.
    pub id: Guid,
    /// The entities and prefab instances.
    pub content: EntityDocument,
    /// The scene settings.
    pub settings: SceneSettings,
}

/// A prefab file: its identity and entities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrefabAsset {
    /// The asset identity from the sidecar.
    pub id: Guid,
    /// The entities and prefab instances.
    pub content: EntityDocument,
}

/// Writes a component record.
fn write_component(component: &ComponentRecord) -> Value {
    let properties = component
        .properties
        .iter()
        .map(|entry| {
            let mut map = object();
            map.insert("key".to_owned(), Value::String(entry.key.clone()));
            if let Value::Object(typed) = write_typed(&entry.value) {
                map.extend(typed);
            }
            Value::Object(map)
        })
        .collect();
    let mut map = object();
    map.insert("id".to_owned(), guid_value(component.id));
    map.insert("type".to_owned(), Value::String(component.kind.clone()));
    map.insert(
        "schema_version".to_owned(),
        Value::from(i64::from(component.schema_version)),
    );
    map.insert("enabled".to_owned(), Value::Bool(component.enabled));
    map.insert("properties".to_owned(), Value::Array(properties));
    Value::Object(map)
}

/// Reads a component record, defaulting missing fields except the type.
fn read_component(source: &Value) -> Result<ComponentRecord, DocumentError> {
    let map = as_object(source, "component")?;
    let kind = as_str(required(map, "type", "component")?, "component.type")?.to_owned();
    let context = format!("component {kind}");
    let version = int_or(map, "schema_version", 1, &context)?;
    let mut properties = Vec::new();
    for entry in items_or_empty(map, "properties", &context)? {
        let entry_map = as_object(entry, &context)?;
        let key = as_str(required(entry_map, "key", &context)?, &context)?.to_owned();
        let value = read_typed(entry, &format!("{context}.{key}"))?;
        properties.push(PropertyEntry { key, value });
    }
    Ok(ComponentRecord {
        id: guid_or_none(map, "id", &context)?,
        kind,
        schema_version: u32::try_from(version)
            .map_err(|_| DocumentError::malformed(format!("{context}: invalid schema_version")))?,
        enabled: bool_or(map, "enabled", true, &context)?,
        properties,
    })
}

/// The fields every node shares: identity, name, parent, order, activity and
/// transform.
fn write_node_fields(
    map: &mut Map<String, Value>,
    id: Guid,
    name: &str,
    parent: Guid,
    order: i32,
    active: bool,
    transform: &fr_transform::Transform,
) {
    map.insert("id".to_owned(), guid_value(id));
    map.insert("name".to_owned(), Value::String(name.to_owned()));
    map.insert("parent".to_owned(), guid_value(parent));
    map.insert("order".to_owned(), Value::from(i64::from(order)));
    map.insert("active".to_owned(), Value::Bool(active));
    map.insert("transform".to_owned(), write_transform(transform));
}

/// The shared fields of a node as read: identity, name, parent, order, activity
/// and transform.
struct NodeFields {
    /// The identity.
    id: Guid,
    /// The display name.
    name: String,
    /// The parent.
    parent: Guid,
    /// The sibling order.
    order: i32,
    /// Whether the node starts active.
    active: bool,
    /// The transform relative to the parent.
    transform: fr_transform::Transform,
}

/// Reads the fields every node shares.
fn read_node_fields(map: &Map<String, Value>, context: &str) -> Result<NodeFields, DocumentError> {
    let order = int_or(map, "order", 0, context)?;
    Ok(NodeFields {
        id: guid_or_none(map, "id", context)?,
        name: string_or_empty(map, "name", context)?,
        parent: guid_or_none(map, "parent", context)?,
        order: i32::try_from(order)
            .map_err(|_| DocumentError::malformed(format!("{context}: invalid order")))?,
        active: bool_or(map, "active", true, context)?,
        transform: read_transform(map.get("transform"), context)?,
    })
}

/// Writes an entity record.
fn write_entity(entity: &EntityRecord) -> Value {
    let mut map = object();
    write_node_fields(
        &mut map,
        entity.id,
        &entity.name,
        entity.parent,
        entity.order,
        entity.active,
        &entity.transform,
    );
    map.insert(
        "groups".to_owned(),
        Value::Array(entity.groups.iter().cloned().map(Value::String).collect()),
    );
    map.insert(
        "components".to_owned(),
        Value::Array(entity.components.iter().map(write_component).collect()),
    );
    Value::Object(map)
}

/// Reads an entity record, defaulting missing fields.
fn read_entity(source: &Value) -> Result<EntityRecord, DocumentError> {
    let map = as_object(source, "entity")?;
    let fields = read_node_fields(map, "entity")?;
    let context = format!("entity {}", fields.name);
    let mut groups = Vec::new();
    for group in items_or_empty(map, "groups", &context)? {
        groups.push(as_str(group, &context)?.to_owned());
    }
    let mut components = Vec::new();
    for component in items_or_empty(map, "components", &context)? {
        components.push(
            read_component(component)
                .map_err(|error| DocumentError::malformed(format!("{context}: {error}")))?,
        );
    }
    Ok(EntityRecord {
        id: fields.id,
        name: fields.name,
        transform: fields.transform,
        parent: fields.parent,
        order: fields.order,
        active: fields.active,
        groups,
        components,
    })
}

/// Writes a prefab property override.
fn write_override(value: &PropertyOverride) -> Value {
    let mut map = object();
    map.insert("entity".to_owned(), guid_value(value.entity));
    map.insert("component".to_owned(), guid_value(value.component));
    map.insert("property".to_owned(), Value::String(value.property.clone()));
    if let Value::Object(typed) = write_typed(&value.value) {
        map.extend(typed);
    }
    Value::Object(map)
}

/// Reads a prefab property override.
fn read_override(source: &Value) -> Result<PropertyOverride, DocumentError> {
    let map = as_object(source, "override")?;
    let property = string_or_empty(map, "property", "override")?;
    Ok(PropertyOverride {
        entity: guid_or_none(map, "entity", "override")?,
        component: guid_or_none(map, "component", "override")?,
        value: read_typed(source, &format!("override {property}"))?,
        property,
    })
}

/// Writes a prefab instance.
fn write_instance(instance: &PrefabInstanceRecord) -> Value {
    let mut map = object();
    write_node_fields(
        &mut map,
        instance.id,
        &instance.name,
        instance.parent,
        instance.order,
        instance.active,
        &instance.transform,
    );
    map.insert("prefab".to_owned(), write_asset_reference(&instance.prefab));
    map.insert(
        "overrides".to_owned(),
        Value::Array(instance.overrides.iter().map(write_override).collect()),
    );
    Value::Object(map)
}

/// Reads a prefab instance.
fn read_instance(source: &Value) -> Result<PrefabInstanceRecord, DocumentError> {
    let map = as_object(source, "instance")?;
    let fields = read_node_fields(map, "instance")?;
    let context = format!("instance {}", fields.name);
    let mut overrides = Vec::new();
    for value in items_or_empty(map, "overrides", &context)? {
        overrides.push(read_override(value)?);
    }
    Ok(PrefabInstanceRecord {
        id: fields.id,
        name: fields.name,
        transform: fields.transform,
        parent: fields.parent,
        order: fields.order,
        active: fields.active,
        prefab: read_asset_reference(map.get("prefab"), &context)?,
        overrides,
    })
}

/// Adds the entities and instances of a document to a file object.
fn write_content(map: &mut Map<String, Value>, content: &EntityDocument) {
    map.insert(
        "entities".to_owned(),
        Value::Array(content.entities.iter().map(write_entity).collect()),
    );
    map.insert(
        "instances".to_owned(),
        Value::Array(content.instances.iter().map(write_instance).collect()),
    );
}

/// Reads the entities and instances of a file object; missing arrays are empty.
fn read_content(map: &Map<String, Value>) -> Result<EntityDocument, DocumentError> {
    let mut content = EntityDocument::default();
    for entity in items_or_empty(map, "entities", "document")? {
        content.entities.push(read_entity(entity)?);
    }
    for instance in items_or_empty(map, "instances", "document")? {
        content.instances.push(read_instance(instance)?);
    }
    Ok(content)
}

/// Starts a file object with its format name and version.
fn start_file(kind: FormatKind) -> Map<String, Value> {
    let mut map = object();
    map.insert("format".to_owned(), Value::String(kind.name().to_owned()));
    map.insert(
        "format_version".to_owned(),
        Value::from(i64::from(kind.version())),
    );
    map
}

/// Checks a file object's format name and version.
///
/// # Errors
///
/// [`DocumentError::WrongFormat`] when the name differs and
/// [`DocumentError::Version`] naming the version found and required when only
/// the version does.
pub fn check_format(
    map: &Map<String, Value>,
    name: &'static str,
    version: u32,
) -> Result<(), DocumentError> {
    let found = string_or_empty(map, "format", "document")?;
    if found != name {
        return Err(DocumentError::WrongFormat {
            required: name,
            found,
        });
    }
    let found_version = map
        .get("format_version")
        .map_or(Ok(0), |value| as_i64(value, "format_version"))?;
    if u32::try_from(found_version).ok() != Some(version) {
        return Err(DocumentError::Version {
            format: name.to_owned(),
            found: u32::try_from(found_version).unwrap_or(0),
            required: version,
        });
    }
    Ok(())
}

/// Parses a file and checks its format name and version.
fn open_file(source: &str, kind: FormatKind) -> Result<Map<String, Value>, DocumentError> {
    let value = json::parse(source)?;
    let map = as_object(&value, "document")?;
    check_format(map, kind.name(), kind.version())?;
    Ok(map.clone())
}

/// Serialises a scene.
pub fn write_scene_text(asset: &SceneAsset) -> String {
    let mut map = start_file(FormatKind::Scene);
    map.insert("settings".to_owned(), write_scene_settings(&asset.settings));
    write_content(&mut map, &asset.content);
    json::write(&Value::Object(map))
}

/// Parses a scene; the identity is left unset.
///
/// # Errors
///
/// When the text is not a scene of the current version.
pub fn read_scene_text(source: &str) -> Result<SceneAsset, DocumentError> {
    let map = open_file(source, FormatKind::Scene)?;
    let settings = match map.get("settings") {
        Some(found) => read_scene_settings(found)?,
        None => SceneSettings::default(),
    };
    Ok(SceneAsset {
        id: Guid::NONE,
        content: read_content(&map)?,
        settings,
    })
}

/// Serialises a prefab.
pub fn write_prefab_text(asset: &PrefabAsset) -> String {
    let mut map = start_file(FormatKind::Prefab);
    write_content(&mut map, &asset.content);
    json::write(&Value::Object(map))
}

/// Parses a prefab; the identity is left unset.
///
/// # Errors
///
/// When the text is not a prefab of the current version.
pub fn read_prefab_text(source: &str) -> Result<PrefabAsset, DocumentError> {
    let map = open_file(source, FormatKind::Prefab)?;
    Ok(PrefabAsset {
        id: Guid::NONE,
        content: read_content(&map)?,
    })
}
