//! Running a command against the content of a document.

use fr_document::components::{make_entity, set_property};
use fr_document::{
    ComponentRecord, EntityRecord, Guid, PrefabInstanceRecord, PropertyValue, SchemaSet,
};

use crate::command::{AddPrefabInstance, Command, ComponentInit, CreateEntity};
use crate::data::DocumentData;
use crate::error::AuthoringError;
use crate::rules::{
    can_remove_component, plan_add_component, transform_is_finite, validate_property_value,
};
use crate::tree::{
    duplicate_subtree, node_fields_mut, place, remove_subtree, require_node, require_parent,
    subtree_ids,
};

/// What a command needs to know beyond the content it edits.
pub struct ApplyContext<'a> {
    /// The component schemas rules are read from.
    pub schemas: &'a SchemaSet,
    /// The asset identity of the document, unset when it has none yet.
    pub document_asset: Guid,
}

/// Applies a command to the content, returning the identities it created.
///
/// # Errors
///
/// The command's rejection. The content may be partly changed on failure, so
/// callers apply to a copy.
pub fn apply_command(
    command: &Command,
    data: &mut DocumentData,
    context: &ApplyContext<'_>,
) -> Result<Vec<Guid>, AuthoringError> {
    let document = &mut data.content;
    match command {
        Command::CreateEntity(spec) => create_entity(document, spec, context).map(|id| vec![id]),
        Command::DeleteNode { node } => remove_subtree(document, *node).map(|_| Vec::new()),
        Command::Duplicate { node } => duplicate_subtree(document, *node).map(|id| vec![id]),
        Command::Reparent {
            node,
            parent,
            index,
        } => reparent(document, *node, *parent, *index).map(|()| Vec::new()),
        Command::Rename { node, name } => rename(document, *node, name).map(|()| Vec::new()),
        Command::SetTransform { node, transform } => {
            if !transform_is_finite(transform) {
                return Err(AuthoringError::invalid_value(
                    "transform",
                    "the transform is not finite",
                ));
            }
            *node_fields_mut(document, *node)?.transform = *transform;
            Ok(Vec::new())
        }
        Command::AddComponent { entity, kind } => {
            let record = entity_mut(document, *entity)?;
            add_component_to(record, kind, context.schemas)
        }
        Command::RemoveComponent { entity, component } => {
            remove_component(entity_mut(document, *entity)?, *component, context.schemas)
                .map(|()| Vec::new())
        }
        Command::SetProperty {
            entity,
            component,
            key,
            value,
        } => {
            let record = entity_mut(document, *entity)?;
            set_component_property(record, *component, key, value, context.schemas)
                .map(|()| Vec::new())
        }
        Command::SetComponentEnabled {
            entity,
            component,
            enabled,
        } => {
            component_mut(entity_mut(document, *entity)?, *component)?.enabled = *enabled;
            Ok(Vec::new())
        }
        Command::SetActive { node, active } => {
            *node_fields_mut(document, *node)?.active = *active;
            Ok(Vec::new())
        }
        Command::AddPrefabInstance(spec) => {
            add_prefab_instance(document, spec, context).map(|id| vec![id])
        }
        Command::SetSceneSettings(settings) => {
            data.settings = *settings;
            Ok(Vec::new())
        }
        Command::Batch { commands, .. } => {
            let mut created = Vec::new();
            for inner in commands {
                created.extend(apply_command(inner, data, context)?);
            }
            Ok(created)
        }
    }
}

/// Checks that a name is not blank.
///
/// # Errors
///
/// [`AuthoringError::InvalidValue`] for an empty or blank name.
fn require_name(name: &str) -> Result<(), AuthoringError> {
    if name.trim().is_empty() {
        Err(AuthoringError::invalid_value("name", "the name is empty"))
    } else {
        Ok(())
    }
}

/// Renames a node.
///
/// # Errors
///
/// When the node is missing or the name blank.
fn rename(
    document: &mut fr_document::EntityDocument,
    node: Guid,
    name: &str,
) -> Result<(), AuthoringError> {
    require_name(name)?;
    name.clone_into(node_fields_mut(document, node)?.name);
    Ok(())
}

/// Moves a node under another parent.
///
/// # Errors
///
/// When the node or parent is missing, or the parent is the node or one of its
/// descendants.
fn reparent(
    document: &mut fr_document::EntityDocument,
    node: Guid,
    parent: Guid,
    index: Option<usize>,
) -> Result<(), AuthoringError> {
    require_node(document, node)?;
    require_parent(document, parent)?;
    if parent.valid() && subtree_ids(document, node).contains(&parent) {
        return Err(AuthoringError::ReparentCycle { node, parent });
    }
    place(document, node, parent, index)
}

/// The entity with an identity.
///
/// # Errors
///
/// [`AuthoringError::WrongNodeKind`] for a prefab instance and
/// [`AuthoringError::NodeNotFound`] when there is no such node.
fn entity_mut(
    document: &mut fr_document::EntityDocument,
    id: Guid,
) -> Result<&mut EntityRecord, AuthoringError> {
    if document.find_instance(id).is_some() {
        return Err(AuthoringError::WrongNodeKind {
            id,
            expected: "an entity",
        });
    }
    document
        .entities
        .iter_mut()
        .find(|entity| entity.id == id)
        .ok_or(AuthoringError::NodeNotFound { id })
}

/// The component of an entity with an identity.
///
/// # Errors
///
/// [`AuthoringError::ComponentNotFound`] when there is none.
fn component_mut(
    entity: &mut EntityRecord,
    component: Guid,
) -> Result<&mut ComponentRecord, AuthoringError> {
    let owner = entity.id;
    entity
        .components
        .iter_mut()
        .find(|candidate| candidate.id == component)
        .ok_or(AuthoringError::ComponentNotFound {
            entity: owner,
            component,
        })
}

/// Adds a component with the prerequisites it needs, returning the identities
/// of the new components with the requested one last.
///
/// # Errors
///
/// [`AuthoringError::UnknownComponentType`] for an unregistered type and
/// [`AuthoringError::ComponentRejected`] with the reason otherwise.
pub fn add_component_to(
    entity: &mut EntityRecord,
    kind: &str,
    schemas: &SchemaSet,
) -> Result<Vec<Guid>, AuthoringError> {
    if schemas.find(kind).is_none() {
        return Err(AuthoringError::UnknownComponentType {
            kind: kind.to_owned(),
        });
    }
    let plan = plan_add_component(entity, kind, schemas)
        .map_err(|reason| AuthoringError::rejected(kind, reason))?;
    let mut created = Vec::new();
    for step in &plan.kinds {
        let Some(component) = crate::rules::default_component(step, schemas) else {
            return Err(AuthoringError::UnknownComponentType { kind: step.clone() });
        };
        created.push(component.id);
        entity.components.push(component);
    }
    Ok(created)
}

/// Removes a component.
///
/// # Errors
///
/// When the component is missing, not removable or still required.
fn remove_component(
    entity: &mut EntityRecord,
    component: Guid,
    schemas: &SchemaSet,
) -> Result<(), AuthoringError> {
    let kind = component_mut(entity, component)?.kind.clone();
    can_remove_component(entity, component, schemas)
        .map_err(|reason| AuthoringError::rejected(&kind, reason))?;
    entity
        .components
        .retain(|candidate| candidate.id != component);
    Ok(())
}

/// Sets a validated property of a component.
///
/// # Errors
///
/// When the component is missing or the value is not valid for the property.
fn set_component_property(
    entity: &mut EntityRecord,
    component: Guid,
    key: &str,
    value: &PropertyValue,
    schemas: &SchemaSet,
) -> Result<(), AuthoringError> {
    let record = component_mut(entity, component)?;
    validate_property_value(record, schemas.find(&record.kind), key, value)?;
    set_property(record, key, value.clone());
    Ok(())
}

/// Adds the components of an initial list, each with its property values.
///
/// # Errors
///
/// When a component cannot be added or a value is not valid.
fn add_initial_components(
    entity: &mut EntityRecord,
    inits: &[ComponentInit],
    schemas: &SchemaSet,
) -> Result<(), AuthoringError> {
    for init in inits {
        let created = add_component_to(entity, &init.kind, schemas)?;
        let Some(requested) = created.last().copied() else {
            continue;
        };
        for (key, value) in &init.properties {
            set_component_property(entity, requested, key, value, schemas)?;
        }
    }
    Ok(())
}

/// Creates an entity with its components and places it.
///
/// # Errors
///
/// When the name is blank, the transform not finite, the parent missing, or a
/// component cannot be added.
fn create_entity(
    document: &mut fr_document::EntityDocument,
    spec: &CreateEntity,
    context: &ApplyContext<'_>,
) -> Result<Guid, AuthoringError> {
    require_name(&spec.name)?;
    require_parent(document, spec.parent)?;
    if !transform_is_finite(&spec.transform) {
        return Err(AuthoringError::invalid_value(
            "transform",
            "the transform is not finite",
        ));
    }
    let mut entity = make_entity(spec.name.clone());
    entity.transform = spec.transform;
    add_initial_components(&mut entity, &spec.components, context.schemas)?;
    let id = entity.id;
    document.entities.push(entity);
    place(document, id, spec.parent, spec.index)?;
    Ok(id)
}

/// The default name of an instance of a prefab: the file stem of its last
/// known path.
fn instance_name(spec: &AddPrefabInstance) -> String {
    let path = spec.prefab.last_known_path.as_str();
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.split('.').next().unwrap_or(file);
    if stem.is_empty() {
        "Prefab".to_owned()
    } else {
        stem.to_owned()
    }
}

/// Places an instance of a prefab.
///
/// # Errors
///
/// When the prefab reference is unset or the document itself, the transform not
/// finite or the parent missing.
fn add_prefab_instance(
    document: &mut fr_document::EntityDocument,
    spec: &AddPrefabInstance,
    context: &ApplyContext<'_>,
) -> Result<Guid, AuthoringError> {
    if !spec.prefab.asset.valid() {
        return Err(AuthoringError::invalid_value(
            "prefab",
            "the prefab asset is not set",
        ));
    }
    if spec.prefab.asset == context.document_asset {
        return Err(AuthoringError::invalid_value(
            "prefab",
            "a prefab cannot contain an instance of itself",
        ));
    }
    require_parent(document, spec.parent)?;
    if !transform_is_finite(&spec.transform) {
        return Err(AuthoringError::invalid_value(
            "transform",
            "the transform is not finite",
        ));
    }
    let name = if spec.name.trim().is_empty() {
        instance_name(spec)
    } else {
        spec.name.clone()
    };
    let id = Guid::generate();
    document.instances.push(PrefabInstanceRecord {
        id,
        name,
        transform: spec.transform,
        prefab: spec.prefab.clone(),
        ..PrefabInstanceRecord::default()
    });
    place(document, id, spec.parent, spec.index)?;
    Ok(id)
}
