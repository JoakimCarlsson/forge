//! Creating components and entities, and the requirement and conflict rules of
//! component types.

use crate::guid::Guid;
use crate::property::{PropertyEntry, PropertyValue};
use crate::records::{ComponentRecord, EntityRecord};
use crate::schema::{ComponentSchema, SchemaSet};

/// A new active root entity with a fresh identity.
pub fn make_entity(name: impl Into<String>) -> EntityRecord {
    EntityRecord {
        id: Guid::generate(),
        name: name.into(),
        ..EntityRecord::default()
    }
}

/// A new component of a type with a fresh identity and the type's defaults.
pub fn make_component(schema: &ComponentSchema) -> ComponentRecord {
    ComponentRecord {
        id: Guid::generate(),
        kind: schema.key.clone(),
        schema_version: schema.schema_version,
        properties: schema
            .properties
            .iter()
            .map(|property| PropertyEntry {
                key: property.key.clone(),
                value: property.default_value.clone(),
            })
            .collect(),
        ..ComponentRecord::default()
    }
}

/// Sets a property of a component, appending it when it is not there.
pub fn set_property(component: &mut ComponentRecord, key: &str, value: PropertyValue) {
    if let Some(entry) = component
        .properties
        .iter_mut()
        .find(|entry| entry.key == key)
    {
        entry.value = value;
        return;
    }
    component.properties.push(PropertyEntry {
        key: key.to_owned(),
        value,
    });
}

/// The types an entity must still gain before a type can be added, nearest
/// requirement first, each listed once.
pub fn missing_requirements(
    entity: &EntityRecord,
    schema: &ComponentSchema,
    registry: &SchemaSet,
) -> Vec<String> {
    let mut missing: Vec<String> = Vec::new();
    for required in &schema.required_types {
        if entity.find_component(required).is_some() {
            continue;
        }
        if !missing.contains(required) {
            missing.push(required.clone());
        }
        let Some(nested) = registry.find(required) else {
            continue;
        };
        for deeper in missing_requirements(entity, nested, registry) {
            if !missing.contains(&deeper) {
                missing.push(deeper);
            }
        }
    }
    missing
}

/// The types of the entity's components that require a type, one per
/// component.
pub fn component_dependents(
    entity: &EntityRecord,
    kind: &str,
    registry: &SchemaSet,
) -> Vec<String> {
    entity
        .components
        .iter()
        .filter(|component| {
            registry.find(&component.kind).is_some_and(|schema| {
                schema
                    .required_types
                    .iter()
                    .any(|required| required == kind)
            })
        })
        .map(|component| component.kind.clone())
        .collect()
}

/// Checks that a type may be added to an entity.
///
/// # Errors
///
/// The reason it may not: the entity already has one and the type is single, or
/// the entity holds a conflicting type.
pub fn can_add_component(entity: &EntityRecord, schema: &ComponentSchema) -> Result<(), String> {
    if schema.intrinsic {
        return Err(format!("{} is part of every entity", schema.label));
    }
    if !schema.allow_multiple && entity.find_component(&schema.key).is_some() {
        return Err(format!("{} is already on this entity", schema.label));
    }
    for conflict in &schema.conflicting_types {
        if entity.find_component(conflict).is_some() {
            return Err(format!(
                "{} cannot be combined with {conflict}",
                schema.label
            ));
        }
    }
    Ok(())
}
