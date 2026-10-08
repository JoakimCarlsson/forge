//! Versioned scene serialization using registered component reflection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fr_app::ecs::{entity::Entity, reflect::ReflectComponent, world::World};
use fr_app::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use fr_app::reflect::{PartialReflect, ReflectFromReflect, TypeRegistry};
use serde::de::DeserializeSeed;
use serde::{Deserialize, Serialize};

use crate::{Parent, SceneId};

/// The supported scene document version.
const SCENE_VERSION: u32 = 1;

/// A failure to encode, validate or instantiate a scene.
#[derive(Debug)]
pub struct SceneError(pub String);

impl fmt::Display for SceneError {
    /// Writes the scene failure.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SceneError {}

/// One authored object and its registered, reflected components.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneObject {
    /// The stable object identifier.
    pub id: String,
    /// The stable parent identifier, when this object is a child.
    pub parent: Option<String>,
    /// Component values indexed by their registered full type paths.
    pub components: BTreeMap<String, serde_json::Value>,
}

/// GPU-independent scene data suitable for storage or visual editing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneDocument {
    /// The schema version used to interpret the document.
    pub version: u32,
    /// The authored objects in persistent identifier order.
    pub objects: Vec<SceneObject>,
}

impl Default for SceneDocument {
    /// Creates an empty document in the current schema.
    fn default() -> Self {
        Self {
            version: SCENE_VERSION,
            objects: Vec::new(),
        }
    }
}

/// A component decoded and validated before the target world changes.
struct PreparedComponent {
    /// The reflection operations registered for the component.
    component: ReflectComponent,
    /// The decoded component value.
    value: Box<dyn PartialReflect>,
}

impl SceneDocument {
    /// Captures objects with stable IDs and registered reflected components.
    ///
    /// Runtime components without reflection metadata are omitted.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid IDs or hierarchy, or components that cannot serialize.
    pub fn capture(world: &mut World, registry: &TypeRegistry) -> Result<Self, SceneError> {
        let mut query = world.query::<(Entity, &SceneId, Option<&Parent>)>();
        let mut objects = Vec::new();
        for (entity, id, parent) in query.iter(world) {
            let entity = world.entity(entity);
            let mut components = BTreeMap::new();
            for registration in registry.iter() {
                if registration.type_id() == std::any::TypeId::of::<SceneId>()
                    || registration.type_id() == std::any::TypeId::of::<Parent>()
                {
                    continue;
                }
                let Some(component) = registration.data::<ReflectComponent>() else {
                    continue;
                };
                if let Some(value) = component.reflect(entity) {
                    let value = serde_json::to_value(TypedReflectSerializer::new(value, registry))
                        .map_err(|error| SceneError(error.to_string()))?;
                    components.insert(registration.type_info().type_path().to_owned(), value);
                }
            }
            objects.push(SceneObject {
                id: id.0.clone(),
                parent: parent.map(|parent| parent.0.0.clone()),
                components,
            });
        }
        objects.sort_by(|a, b| a.id.cmp(&b.id));
        let document = Self {
            objects,
            ..Self::default()
        };
        document.validate()?;
        Ok(document)
    }

    /// Encodes this document as readable JSON.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid document or an encoding failure.
    pub fn to_json(&self) -> Result<String, SceneError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|error| SceneError(error.to_string()))
    }

    /// Decodes and validates a scene document.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed JSON, an unsupported version or an invalid hierarchy.
    pub fn from_json(source: &str) -> Result<Self, SceneError> {
        let document: Self =
            serde_json::from_str(source).map_err(|error| SceneError(error.to_string()))?;
        document.validate()?;
        Ok(document)
    }

    /// Instantiates a validated scene, retaining persistent IDs and parent references.
    ///
    /// # Errors
    ///
    /// Returns an error before spawning objects if IDs conflict with the target world,
    /// component types are unknown or component data cannot be decoded.
    pub fn instantiate(
        &self,
        world: &mut World,
        registry: &TypeRegistry,
    ) -> Result<BTreeMap<SceneId, Entity>, SceneError> {
        self.validate()?;
        let mut query = world.query::<&SceneId>();
        let existing: BTreeSet<_> = query.iter(world).map(|id| id.0.as_str()).collect();
        let mut prepared = Vec::new();
        for object in &self.objects {
            if existing.contains(object.id.as_str()) {
                return Err(SceneError(format!(
                    "object identifier {} already exists",
                    object.id
                )));
            }
            let mut components = Vec::new();
            for (path, value) in &object.components {
                let registration = registry
                    .get_with_type_path(path)
                    .ok_or_else(|| SceneError(format!("unknown component type {path}")))?;
                if registration.type_id() == std::any::TypeId::of::<SceneId>()
                    || registration.type_id() == std::any::TypeId::of::<Parent>()
                {
                    return Err(SceneError(format!("{path} belongs in the object header")));
                }
                let component = registration
                    .data::<ReflectComponent>()
                    .ok_or_else(|| SceneError(format!("{path} is not a reflected component")))?;
                let value = TypedReflectDeserializer::new(registration, registry)
                    .deserialize(value.clone())
                    .map_err(|error| SceneError(error.to_string()))?;
                let from_reflect = registration.data::<ReflectFromReflect>().ok_or_else(|| {
                    SceneError(format!("{path} cannot construct a reflected value"))
                })?;
                let value = from_reflect
                    .from_reflect(value.as_ref())
                    .ok_or_else(|| SceneError(format!("invalid component value for {path}")))?;
                components.push(PreparedComponent {
                    component: component.clone(),
                    value: value.into_partial_reflect(),
                });
            }
            prepared.push(components);
        }
        let mut entities = BTreeMap::new();
        for (object, components) in self.objects.iter().zip(prepared) {
            let id = SceneId(object.id.clone());
            let mut entity = world.spawn(id.clone());
            if let Some(parent) = &object.parent {
                entity.insert(Parent(SceneId(parent.clone())));
            }
            for component in components {
                component
                    .component
                    .insert(&mut entity, component.value.as_ref(), registry);
            }
            entities.insert(id, entity.id());
        }
        Ok(entities)
    }

    /// Validates the schema, identifiers and parent graph.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported versions, empty or duplicate IDs, missing parents
    /// or cycles in the parent graph.
    pub fn validate(&self) -> Result<(), SceneError> {
        if self.version != SCENE_VERSION {
            return Err(SceneError(format!(
                "unsupported scene version {}",
                self.version
            )));
        }
        let mut parents = BTreeMap::new();
        for object in &self.objects {
            if object.id.is_empty() || parents.insert(&object.id, object.parent.as_ref()).is_some()
            {
                return Err(SceneError(format!(
                    "empty or duplicate object identifier {}",
                    object.id
                )));
            }
        }
        let mut resolved = BTreeSet::new();
        for id in parents.keys() {
            let mut path = BTreeSet::new();
            let mut next = Some(*id);
            while let Some(id) = next {
                if resolved.contains(id) {
                    break;
                }
                if !path.insert(id) {
                    return Err(SceneError(String::from(
                        "the object hierarchy contains a cycle",
                    )));
                }
                next = *parents
                    .get(id)
                    .ok_or_else(|| SceneError(format!("missing parent {id}")))?;
            }
            resolved.extend(path);
        }
        Ok(())
    }
}
