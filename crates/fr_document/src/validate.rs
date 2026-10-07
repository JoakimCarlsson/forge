//! Checking a document against the registered component types: identifiers,
//! parents, cycles, prefab references, schema versions, property types,
//! multiplicity and dependency rules.

use std::collections::HashSet;

use crate::guid::Guid;
use crate::nodes::MAX_DEPTH;
use crate::records::{ComponentRecord, EntityDocument, EntityRecord};
use crate::schema::{Diagnostic, SchemaSet, Severity};

/// A diagnostic of an entity.
fn about_entity(severity: Severity, entity: Guid, message: String) -> Diagnostic {
    Diagnostic {
        severity,
        message,
        entity,
        component: Guid::NONE,
    }
}

/// Checks one component against its registered type.
fn validate_component(
    entity: &EntityRecord,
    component: &ComponentRecord,
    set: &SchemaSet,
    out: &mut Vec<Diagnostic>,
) {
    let report = |severity: Severity, message: String| Diagnostic {
        severity,
        message,
        entity: entity.id,
        component: component.id,
    };
    let Some(schema) = set.find(&component.kind) else {
        out.push(report(
            Severity::Warning,
            format!(
                "component type {} is unavailable; its data is preserved but inert",
                component.kind
            ),
        ));
        return;
    };
    if schema.intrinsic {
        out.push(report(
            Severity::Error,
            format!(
                "{} is part of the entity and cannot be a component",
                component.kind
            ),
        ));
        return;
    }
    if schema.schema_version != component.schema_version {
        out.push(report(
            Severity::Error,
            format!(
                "component {} has schema version {} but {} is required",
                component.kind, component.schema_version, schema.schema_version
            ),
        ));
    }
    if !schema.allow_multiple
        && entity
            .components
            .iter()
            .filter(|other| other.kind == component.kind)
            .count()
            > 1
    {
        out.push(report(
            Severity::Error,
            format!("{} may appear only once on an entity", component.kind),
        ));
    }
    for entry in &component.properties {
        match schema.find_property(&entry.key) {
            None => out.push(report(
                Severity::Warning,
                format!("property {} is not part of {}", entry.key, component.kind),
            )),
            Some(property) if property.kind != entry.value.kind() => out.push(report(
                Severity::Error,
                format!(
                    "property {} of {} has the wrong value type",
                    entry.key, component.kind
                ),
            )),
            Some(_) => {}
        }
    }
    for required in &schema.required_types {
        if entity.find_component(required).is_none() {
            out.push(report(
                Severity::Error,
                format!("{} requires {required} on the same entity", component.kind),
            ));
        }
    }
    for conflict in &schema.conflicting_types {
        if entity.find_component(conflict).is_some() {
            out.push(report(
                Severity::Error,
                format!("{} cannot be combined with {conflict}", component.kind),
            ));
        }
    }
}

/// Reports an identifier that is unset or already taken.
fn claim(seen: &mut HashSet<Guid>, id: Guid, what: String, owner: Guid, out: &mut Vec<Diagnostic>) {
    if !id.valid() {
        out.push(about_entity(
            Severity::Error,
            owner,
            format!("{what} has no identifier"),
        ));
    } else if !seen.insert(id) {
        out.push(about_entity(
            Severity::Error,
            owner,
            format!("{what} reuses identifier {}", id.to_text()),
        ));
    }
}

/// Checks the identifiers of every node and component of a document.
fn validate_identifiers(document: &EntityDocument, out: &mut Vec<Diagnostic>) {
    let mut seen: HashSet<Guid> = HashSet::new();
    for entity in &document.entities {
        claim(
            &mut seen,
            entity.id,
            format!("entity {}", entity.name),
            entity.id,
            out,
        );
        for component in &entity.components {
            claim(
                &mut seen,
                component.id,
                format!("component {}", component.kind),
                entity.id,
                out,
            );
        }
    }
    for instance in &document.instances {
        claim(
            &mut seen,
            instance.id,
            format!("prefab instance {}", instance.name),
            instance.id,
            out,
        );
    }
}

/// Checks a document: identifiers, parents, cycles, prefab references and every
/// component against its registered type.
pub fn validate_document(document: &EntityDocument, set: &SchemaSet) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    validate_identifiers(document, &mut out);
    for entity in &document.entities {
        if entity.parent.valid() && !document.has_node(entity.parent) {
            out.push(about_entity(
                Severity::Error,
                entity.id,
                format!("entity {} has a missing parent", entity.name),
            ));
        }
        if document.node_depth(entity.id) >= MAX_DEPTH {
            out.push(about_entity(
                Severity::Error,
                entity.id,
                format!("entity {} is part of a parent cycle", entity.name),
            ));
        }
        for component in &entity.components {
            validate_component(entity, component, set, &mut out);
        }
    }
    for instance in &document.instances {
        if instance.parent.valid() && !document.has_node(instance.parent) {
            out.push(about_entity(
                Severity::Error,
                instance.id,
                format!("prefab instance {} has a missing parent", instance.name),
            ));
        }
        if !instance.prefab.asset.valid() {
            out.push(about_entity(
                Severity::Error,
                instance.id,
                format!(
                    "prefab instance {} does not reference a prefab asset",
                    instance.name
                ),
            ));
        }
    }
    out
}
