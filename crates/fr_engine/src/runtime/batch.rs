//! Constructing the behaviours of one realization: construct, apply the
//! authored properties and references, validate the batch, initialize, enable.

use fr_document::{ComponentRecord, ComponentSchema, PropertyValue};
use fr_scene::{Realization, RealizedBehaviour};

use super::{Runtime, Slot};
use crate::behavior::BehaviorId;
use crate::error::{EngineError, EngineResult};
use crate::services::Services;

/// The properties of a record as a behaviour receives them: every property of
/// its schema, the record's value when it has the schema's type and the
/// schema's default otherwise.
fn effective_properties(
    schema: &ComponentSchema,
    record: &ComponentRecord,
) -> Vec<(String, PropertyValue)> {
    schema
        .properties
        .iter()
        .map(|property| {
            let value = match record.property(&property.key) {
                Some(value) if value.kind() == property.kind => value.clone(),
                _ => property.default_value.clone(),
            };
            (property.key.clone(), value)
        })
        .collect()
}

impl Runtime {
    /// Constructs and initializes the behaviours of a realization. A failure
    /// destroys the behaviours of the batch, newest first, and reports its
    /// message; the caller destroys the entities.
    pub(crate) fn adopt(
        &mut self,
        services: &mut Services<'_>,
        built: &Realization,
    ) -> EngineResult {
        let first = self.slots.len();
        let outcome = self.construct_batch(services, built);
        if outcome.is_err() {
            let ids: Vec<BehaviorId> = self.slots[first..].iter().map(|slot| slot.id).collect();
            let _ = self.drop_behaviors(services, &ids);
        }
        outcome
    }

    /// Runs the stages of construction over every behaviour of a realization,
    /// each stage over the whole batch before the next begins.
    fn construct_batch(
        &mut self,
        services: &mut Services<'_>,
        built: &Realization,
    ) -> EngineResult {
        let mut ids = Vec::with_capacity(built.behaviours.len());
        for item in &built.behaviours {
            ids.push(self.construct(item)?);
        }
        for (item, id) in built.behaviours.iter().zip(&ids) {
            self.apply(services, built, item, *id)?;
        }
        for id in &ids {
            self.call(services, *id, |behavior, context| {
                behavior.validate(context)
            })
            .map_err(|error| EngineError::new(format!("validation failed: {error}")))?;
        }
        for id in &ids {
            self.call(services, *id, |behavior, context| {
                behavior.initialize(context)
            })?;
            if let Some(slot) = self.slot_mut(*id) {
                slot.initialized = true;
            }
        }
        self.refresh_activation(services)
    }

    /// Makes the behaviour of a component through its factory and files it.
    fn construct(&mut self, item: &RealizedBehaviour) -> Result<BehaviorId, EngineError> {
        let kind = &item.record.kind;
        let factory = self.registry.factory(kind).ok_or_else(|| {
            EngineError::new(format!("component type {kind} has no registered behaviour"))
        })?;
        let behavior = factory();
        let id = self.next_id;
        self.next_id += 1;
        self.slots.push(Slot {
            id,
            entity: item.entity,
            component: item.record.id,
            kind: kind.clone(),
            enabled: item.record.enabled,
            active: false,
            initialized: false,
            behavior: Some(behavior),
        });
        Ok(id)
    }

    /// Hands a behaviour its properties; entity and component references are
    /// resolved inside the prefab instances the behaviour is in first.
    fn apply(
        &mut self,
        services: &mut Services<'_>,
        built: &Realization,
        item: &RealizedBehaviour,
        id: BehaviorId,
    ) -> EngineResult {
        let Some(schema) = self.registry.find(&item.record.kind).cloned() else {
            return Ok(());
        };
        for (key, value) in effective_properties(&schema, &item.record) {
            self.call(services, id, |behavior, context| match &value {
                PropertyValue::Entity(reference) => {
                    let target = built
                        .resolve_entity(&item.path, reference)
                        .unwrap_or(fr_scene::EntityHandle::NULL);
                    behavior.apply_entity(context, &key, target)
                }
                PropertyValue::Component(reference) => {
                    let owner = built
                        .resolve_entity(&item.path, &reference.owner)
                        .unwrap_or(fr_scene::EntityHandle::NULL);
                    behavior.apply_component(context, &key, owner, reference.component)
                }
                other => behavior.apply_property(context, &key, other),
            })?;
        }
        Ok(())
    }
}
