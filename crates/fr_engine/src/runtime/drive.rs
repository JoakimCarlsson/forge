//! Calling behaviours: one callback at a time with its context, activation and
//! the updates of a step and a frame.

use fr_scene::EntityHandle;

use super::Runtime;
use crate::behavior::{Behavior, BehaviorContext, BehaviorId, Identity};
use crate::error::EngineResult;
use crate::services::Services;
use crate::timing::{FixedFrameInfo, FrameInfo};

impl Runtime {
    /// Runs one callback of a behaviour with its context. The behaviour is out
    /// of its slot while the callback runs, so nothing reaches it re-entrantly.
    /// A behaviour that no longer exists is skipped.
    pub(crate) fn call(
        &mut self,
        services: &mut Services<'_>,
        id: BehaviorId,
        callback: impl FnOnce(&mut dyn Behavior, &mut BehaviorContext<'_>) -> EngineResult,
    ) -> EngineResult {
        let Some(position) = self.position(id) else {
            return Ok(());
        };
        let Some(slot) = self.slots.get_mut(position) else {
            return Ok(());
        };
        let Some(mut behavior) = slot.behavior.take() else {
            return Ok(());
        };
        let identity = Identity {
            id,
            entity: slot.entity,
            component: slot.component,
            enabled: slot.enabled,
        };
        let mut context = BehaviorContext::new(
            identity,
            &mut *services.scene,
            &mut *self,
            &mut *services.assets,
            services.input,
        );
        let outcome = callback(behavior.as_mut(), &mut context);
        if let Some(slot) = self.slot_mut(id) {
            slot.behavior = Some(behavior);
        }
        outcome
    }

    /// Whether a behaviour is active and its type is not paused.
    fn runs(&self, id: BehaviorId) -> bool {
        self.position(id)
            .and_then(|position| self.slots.get(position))
            .is_some_and(|slot| slot.active && slot.initialized && self.kind_enabled(&slot.kind))
    }

    /// Enables the behaviours whose entity and flag now allow it and disables
    /// those that no longer do.
    pub(crate) fn refresh_activation(&mut self, services: &mut Services<'_>) -> EngineResult {
        for id in self.ids() {
            let Some(slot) = self.position(id).and_then(|index| self.slots.get(index)) else {
                continue;
            };
            if !slot.initialized {
                continue;
            }
            let wanted = slot.enabled && services.scene.active_in_hierarchy(slot.entity);
            if wanted == slot.active {
                continue;
            }
            if let Some(slot) = self.slot_mut(id) {
                slot.active = wanted;
            }
            if wanted {
                self.call(services, id, |behavior, context| behavior.enable(context))?;
            } else {
                self.call(services, id, |behavior, context| behavior.disable(context))?;
            }
        }
        Ok(())
    }

    /// Runs the active behaviours' fixed update. Call it after the
    /// application's fixed update and before the physics steps.
    ///
    /// # Errors
    ///
    /// The failure of a behaviour.
    pub fn fixed_update(
        &mut self,
        services: &mut Services<'_>,
        frame: &FixedFrameInfo,
    ) -> EngineResult {
        for id in self.ids() {
            if self.runs(id) {
                self.call(services, id, |behavior, context| {
                    behavior.fixed_update(context, frame)
                })?;
            }
        }
        Ok(())
    }

    /// Refreshes activation, then runs the behaviours' update. Call it after
    /// the application's update.
    ///
    /// # Errors
    ///
    /// The failure of a behaviour.
    pub fn update(&mut self, services: &mut Services<'_>, frame: &FrameInfo) -> EngineResult {
        self.refresh_activation(services)?;
        for id in self.ids() {
            if self.runs(id) {
                self.call(services, id, |behavior, context| {
                    behavior.update(context, frame)
                })?;
            }
        }
        Ok(())
    }

    /// Disables and destroys the behaviours of the entities in a list, leaving
    /// everything else running. The first failure is returned after every
    /// behaviour of the list is gone.
    pub(crate) fn drop_behaviors_on(
        &mut self,
        services: &mut Services<'_>,
        entities: &[EntityHandle],
    ) -> EngineResult {
        let ids: Vec<BehaviorId> = self
            .slots
            .iter()
            .filter(|slot| entities.contains(&slot.entity))
            .map(|slot| slot.id)
            .collect();
        self.drop_behaviors(services, &ids)
    }

    /// Disables and destroys the behaviours with the ids and removes them.
    pub(crate) fn drop_behaviors(
        &mut self,
        services: &mut Services<'_>,
        ids: &[BehaviorId],
    ) -> EngineResult {
        let mut first_failure = Ok(());
        for id in ids {
            let (active, initialized) = self
                .position(*id)
                .and_then(|index| self.slots.get(index))
                .map_or((false, false), |slot| (slot.active, slot.initialized));
            if active {
                let outcome =
                    self.call(services, *id, |behavior, context| behavior.disable(context));
                first_failure = first_failure.and(outcome);
            }
            if initialized {
                let outcome =
                    self.call(services, *id, |behavior, context| behavior.destroy(context));
                first_failure = first_failure.and(outcome);
            }
        }
        self.slots.retain(|slot| !ids.contains(&slot.id));
        first_failure
    }

    /// Disables and destroys every behaviour, leaving the entities in the
    /// scene. A game calls it before dropping the runtime so that behaviours
    /// see their own destruction.
    ///
    /// # Errors
    ///
    /// The first failure of a behaviour while it is torn down.
    pub fn teardown(&mut self, services: &mut Services<'_>) -> EngineResult {
        let ids = self.ids();
        self.drop_behaviors(services, &ids)
    }
}
