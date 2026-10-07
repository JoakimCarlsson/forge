//! The editing rules over components: what can be added or removed, the
//! defaults of a new component and the rows an inspector shows. Every answer
//! comes from the component schemas; nothing here knows a component type.

use fr_document::components::{
    can_add_component, component_dependents, make_component, missing_requirements,
};
use fr_document::{
    ComponentRecord, ComponentSchema, EntityRecord, EnumOption, Guid, PropertySchema, PropertyType,
    PropertyValue, SchemaSet,
};

use crate::error::AuthoringError;

/// One component type an entity could gain, with whether it may be added now.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentChoice {
    /// The type key.
    pub kind: String,
    /// The display label.
    pub label: String,
    /// The category the type is listed under.
    pub category: String,
    /// Ok when the type can be added, otherwise the reason it cannot.
    pub availability: Result<(), String>,
}

/// The components an add creates: the missing prerequisites in dependency
/// order, then the requested type last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddPlan {
    /// The type keys in the order the components are created.
    pub kinds: Vec<String>,
}

impl AddPlan {
    /// The prerequisites that will be added with the requested type.
    pub fn prerequisites(&self) -> &[String] {
        self.kinds.split_last().map_or(&[], |(_, rest)| rest)
    }

    /// The type that was asked for.
    pub fn requested(&self) -> Option<&String> {
        self.kinds.last()
    }
}

/// Every non-intrinsic component type of the registry with whether the entity
/// can gain it, in registry order.
pub fn addable_components(entity: &EntityRecord, schemas: &SchemaSet) -> Vec<ComponentChoice> {
    schemas
        .types()
        .iter()
        .filter(|schema| !schema.intrinsic)
        .map(|schema| ComponentChoice {
            kind: schema.key.clone(),
            label: schema.label.clone(),
            category: schema.category.clone(),
            availability: plan_add_component(entity, &schema.key, schemas).map(|_| ()),
        })
        .collect()
}

/// The plan for adding a type to an entity: missing prerequisites first, each
/// after its own requirements, then the type itself.
///
/// # Errors
///
/// The reason the type cannot be added: it is unregistered, intrinsic, a
/// duplicate of a single component, in conflict with a component the entity
/// holds or that the plan adds, or it requires an unregistered type.
pub fn plan_add_component(
    entity: &EntityRecord,
    kind: &str,
    schemas: &SchemaSet,
) -> Result<AddPlan, String> {
    let schema = schemas
        .find(kind)
        .ok_or_else(|| format!("{kind} is not a registered component type"))?;
    can_add_component(entity, schema)?;
    let mut kinds = missing_requirements(entity, schema, schemas);
    kinds.reverse();
    kinds.push(kind.to_owned());
    let mut simulated = entity.clone();
    for step in &kinds {
        let step_schema = schemas
            .find(step)
            .ok_or_else(|| format!("{kind} requires {step}, which is not registered"))?;
        can_add_component(&simulated, step_schema).map_err(|reason| {
            if step == kind {
                reason
            } else {
                format!("{kind} needs {step}: {reason}")
            }
        })?;
        simulated.components.push(make_component(step_schema));
    }
    Ok(AddPlan { kinds })
}

/// The types of the entity's other components that still require the
/// component's type once it is gone; empty when another component of the same
/// type remains.
pub fn removal_blockers(
    entity: &EntityRecord,
    component: Guid,
    schemas: &SchemaSet,
) -> Vec<String> {
    let Some(target) = entity.components.iter().find(|found| found.id == component) else {
        return Vec::new();
    };
    let another_remains = entity
        .components
        .iter()
        .any(|other| other.kind == target.kind && other.id != target.id);
    if another_remains {
        return Vec::new();
    }
    component_dependents(entity, &target.kind, schemas)
}

/// Checks that a component can be removed from an entity.
///
/// # Errors
///
/// The reason it cannot: the component is missing, its type is not removable,
/// or other components of the entity still require it.
pub fn can_remove_component(
    entity: &EntityRecord,
    component: Guid,
    schemas: &SchemaSet,
) -> Result<(), String> {
    let found = entity
        .components
        .iter()
        .find(|candidate| candidate.id == component)
        .ok_or_else(|| "the entity has no such component".to_owned())?;
    if let Some(schema) = schemas.find(&found.kind)
        && !schema.removable
    {
        return Err(format!("{} cannot be removed", schema.label));
    }
    let blockers = removal_blockers(entity, component, schemas);
    if blockers.is_empty() {
        return Ok(());
    }
    let label = schemas
        .find(&found.kind)
        .map_or(found.kind.as_str(), |schema| schema.label.as_str());
    Err(format!("{label} is required by {}", blockers.join(", ")))
}

/// A new component of a registered type with a fresh identity and the schema's
/// defaults.
pub fn default_component(kind: &str, schemas: &SchemaSet) -> Option<ComponentRecord> {
    schemas.find(kind).map(make_component)
}

/// The data an inspector needs to draw one property: its label, key, type,
/// current value and, when the schema is known, the schema with its options and
/// range.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyRow {
    /// The display label; the key for a property the schema does not list.
    pub label: String,
    /// The property key.
    pub key: String,
    /// The type of the value.
    pub kind: PropertyType,
    /// The current value.
    pub value: PropertyValue,
    /// The schema of the property; none for data whose schema is unavailable.
    pub schema: Option<PropertySchema>,
}

impl PropertyRow {
    /// The options of an enum property; empty otherwise.
    pub fn options(&self) -> &[EnumOption] {
        self.schema
            .as_ref()
            .map_or(&[], |schema| schema.options.as_slice())
    }

    /// The least and greatest value of a number, when the schema limits it.
    pub fn range(&self) -> Option<(f32, f32)> {
        self.schema
            .as_ref()
            .filter(|schema| schema.minimum != schema.maximum)
            .map(|schema| (schema.minimum, schema.maximum))
    }

    /// The editing step of a number; none without a schema.
    pub fn step(&self) -> Option<f32> {
        self.schema.as_ref().map(|schema| schema.step)
    }

    /// Whether the property belongs to a schema that is available.
    pub fn is_known(&self) -> bool {
        self.schema.is_some()
    }
}

/// The rows of a component: the schema's properties in schema order, with the
/// record's value or the default, then the properties only the record has.
pub fn property_rows(component: &ComponentRecord, schemas: &SchemaSet) -> Vec<PropertyRow> {
    let schema = schemas.find(&component.kind);
    let mut rows: Vec<PropertyRow> = Vec::new();
    if let Some(schema) = schema {
        for property in &schema.properties {
            let value = component
                .property(&property.key)
                .cloned()
                .unwrap_or_else(|| property.default_value.clone());
            rows.push(PropertyRow {
                label: property.label.clone(),
                key: property.key.clone(),
                kind: property.kind,
                value,
                schema: Some(property.clone()),
            });
        }
    }
    for entry in &component.properties {
        if schema.is_some_and(|known| known.find_property(&entry.key).is_some()) {
            continue;
        }
        rows.push(PropertyRow {
            label: entry.key.clone(),
            key: entry.key.clone(),
            kind: entry.value.kind(),
            value: entry.value.clone(),
            schema: None,
        });
    }
    rows
}

/// One component of an entity as an inspector section: header data, whether it
/// can be removed and its property rows.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentSection {
    /// The component identity.
    pub component: Guid,
    /// The type key.
    pub kind: String,
    /// The display label; the key for a type whose schema is unavailable.
    pub label: String,
    /// Whether the component takes part in the scene.
    pub enabled: bool,
    /// Whether the type's schema is available.
    pub known: bool,
    /// Ok when the component can be removed, otherwise the reason it cannot.
    pub removal: Result<(), String>,
    /// The property rows.
    pub rows: Vec<PropertyRow>,
}

/// The inspector sections of an entity's components in authored order.
pub fn component_sections(entity: &EntityRecord, schemas: &SchemaSet) -> Vec<ComponentSection> {
    entity
        .components
        .iter()
        .map(|component| {
            let schema = schemas.find(&component.kind);
            ComponentSection {
                component: component.id,
                kind: component.kind.clone(),
                label: schema.map_or_else(|| component.kind.clone(), |found| found.label.clone()),
                enabled: component.enabled,
                known: schema.is_some(),
                removal: can_remove_component(entity, component.id, schemas),
                rows: property_rows(component, schemas),
            }
        })
        .collect()
}

/// Checks that a value may be stored in a property of a component.
///
/// The property's type comes from the schema, or from the record's existing
/// entry when the schema is unavailable.
///
/// # Errors
///
/// [`AuthoringError::UnknownProperty`] when neither lists the key,
/// [`AuthoringError::PropertyTypeMismatch`] for another type, and
/// [`AuthoringError::InvalidValue`] for a non-finite number, a number outside
/// the schema's range, an unlisted enum option or too many array elements.
pub fn validate_property_value(
    component: &ComponentRecord,
    schema: Option<&ComponentSchema>,
    key: &str,
    value: &PropertyValue,
) -> Result<(), AuthoringError> {
    let property = schema.and_then(|found| found.find_property(key));
    let existing = component.property(key);
    let expected = property
        .map(|found| found.kind)
        .or_else(|| existing.map(PropertyValue::kind))
        .ok_or_else(|| AuthoringError::UnknownProperty {
            kind: component.kind.clone(),
            key: key.to_owned(),
        })?;
    if value.kind() != expected {
        return Err(AuthoringError::PropertyTypeMismatch {
            key: key.to_owned(),
            expected,
            found: value.kind(),
        });
    }
    if !is_finite(value) {
        return Err(AuthoringError::invalid_value(
            key,
            "the value is not finite",
        ));
    }
    match property {
        Some(property) => validate_against_schema(property, value),
        None => Ok(()),
    }
}

/// Checks the schema's options, range and array limits for a value of the right
/// type.
fn validate_against_schema(
    property: &PropertySchema,
    value: &PropertyValue,
) -> Result<(), AuthoringError> {
    let key = property.key.as_str();
    match value {
        PropertyValue::Enum(option) if !property.options.is_empty() => {
            if property.options.iter().any(|known| &known.key == option) {
                Ok(())
            } else {
                Err(AuthoringError::invalid_value(
                    key,
                    format!("{option} is not one of the options"),
                ))
            }
        }
        PropertyValue::Int(_) | PropertyValue::Float(_) => {
            let number = value
                .as_float()
                .or_else(|| value.as_int().map(|n| n as f32));
            match number {
                Some(number)
                    if property.minimum != property.maximum
                        && !(property.minimum..=property.maximum).contains(&number) =>
                {
                    Err(AuthoringError::invalid_value(
                        key,
                        format!(
                            "{number} is outside {} to {}",
                            property.minimum, property.maximum
                        ),
                    ))
                }
                _ => Ok(()),
            }
        }
        PropertyValue::Array(items) => {
            if property.max_items > 0 && items.len() > property.max_items as usize {
                return Err(AuthoringError::invalid_value(
                    key,
                    format!("at most {} elements", property.max_items),
                ));
            }
            match items
                .iter()
                .find(|item| item.kind() != property.element_type)
            {
                Some(item) => Err(AuthoringError::PropertyTypeMismatch {
                    key: key.to_owned(),
                    expected: property.element_type,
                    found: item.kind(),
                }),
                None => Ok(()),
            }
        }
        _ => Ok(()),
    }
}

/// Whether every number in a value is finite.
pub(crate) fn is_finite(value: &PropertyValue) -> bool {
    match value {
        PropertyValue::Float(number) => number.is_finite(),
        PropertyValue::Vec3(vector) => vector.is_finite(),
        PropertyValue::Color(color) => [color.r, color.g, color.b, color.a]
            .iter()
            .all(|channel| channel.is_finite()),
        PropertyValue::Rotation(rotation) => rotation.is_finite(),
        PropertyValue::Transform(transform) => transform_is_finite(transform),
        PropertyValue::Array(items) => items.iter().all(is_finite),
        _ => true,
    }
}

/// Whether every component of a transform is finite.
pub(crate) fn transform_is_finite(transform: &fr_transform::Transform) -> bool {
    transform.translation.is_finite()
        && transform.rotation.is_finite()
        && transform.scale.is_finite()
}
