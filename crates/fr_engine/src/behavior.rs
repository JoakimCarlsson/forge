//! Game logic attached to an entity as a component, and the context every
//! callback receives in place of back-pointers into the engine.

use std::any::Any;

use fr_assets::AssetLibrary;
use fr_document::{Guid, PropertyValue};
use fr_input::InputState;
use fr_scene::EntityHandle;

use crate::Scene;
use crate::error::EngineResult;
use crate::runtime::Runtime;
use crate::timing::{FixedFrameInfo, FrameInfo};

/// Names one behaviour for as long as it lives.
pub type BehaviorId = u64;

/// Which behaviour a context belongs to.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Identity {
    /// The behaviour's id.
    pub(crate) id: BehaviorId,
    /// The entity the behaviour is on.
    pub(crate) entity: EntityHandle,
    /// The component the behaviour implements.
    pub(crate) component: Guid,
    /// Whether the behaviour asks to be enabled.
    pub(crate) enabled: bool,
}

/// What a behaviour sees while one of its callbacks runs: the scene, the runtime
/// that owns every other behaviour, the assets and the input, and its own
/// identity.
///
/// The behaviour being called is not among the runtime's behaviours while its
/// callback runs, so nothing the runtime does reaches it re-entrantly.
pub struct BehaviorContext<'a> {
    /// The scene the behaviour is part of.
    pub scene: &'a mut Scene,
    /// The runtime that owns the behaviours.
    pub runtime: &'a mut Runtime,
    /// The assets of the project.
    pub assets: &'a mut AssetLibrary,
    /// The input state of the frame.
    pub input: &'a InputState,
    /// Which behaviour this is.
    identity: Identity,
}

impl<'a> BehaviorContext<'a> {
    /// Creates the context of a behaviour.
    pub(crate) const fn new(
        identity: Identity,
        scene: &'a mut Scene,
        runtime: &'a mut Runtime,
        assets: &'a mut AssetLibrary,
        input: &'a InputState,
    ) -> Self {
        Self {
            scene,
            runtime,
            assets,
            input,
            identity,
        }
    }

    /// The identity of the behaviour.
    pub const fn id(&self) -> BehaviorId {
        self.identity.id
    }

    /// The entity the behaviour is on.
    pub const fn entity(&self) -> EntityHandle {
        self.identity.entity
    }

    /// The identifier of the component the behaviour implements.
    pub const fn component_id(&self) -> Guid {
        self.identity.component
    }

    /// Whether the behaviour asks to be enabled. It runs only while its entity
    /// is also active in the hierarchy.
    pub const fn enabled(&self) -> bool {
        self.identity.enabled
    }

    /// Asks for the behaviour to be enabled or disabled at the next activation
    /// refresh.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.identity.enabled = enabled;
        self.runtime.set_enabled(self.identity.id, enabled);
    }
}

/// Game logic attached to an entity as a component.
///
/// The runtime constructs a behaviour from the factory registered for its
/// component type, applies its authored properties and references, validates
/// the batch, initializes it once and enables and disables it as its entity's
/// activity and its own flag change. Constructors have no scene side effects.
/// Initialization does not imply that another behaviour has initialized.
///
/// A callback that fails stops what the runtime was doing: while a scene loads
/// it rolls the load back, and from an update it ends the frame loop.
pub trait Behavior: Any {
    /// Receives an authored property value that is not a reference.
    fn apply_property(
        &mut self,
        _context: &mut BehaviorContext<'_>,
        _key: &str,
        _value: &PropertyValue,
    ) -> EngineResult {
        Ok(())
    }

    /// Receives an authored entity reference, already resolved; the null handle
    /// when it could not be resolved.
    fn apply_entity(
        &mut self,
        _context: &mut BehaviorContext<'_>,
        _key: &str,
        _target: EntityHandle,
    ) -> EngineResult {
        Ok(())
    }

    /// Receives an authored component reference: the entity owning the
    /// component, already resolved, and the component's identifier.
    fn apply_component(
        &mut self,
        _context: &mut BehaviorContext<'_>,
        _key: &str,
        _owner: EntityHandle,
        _component: Guid,
    ) -> EngineResult {
        Ok(())
    }

    /// Checks the applied properties before initialization. An error rolls the
    /// whole batch back with its message.
    fn validate(&self, _context: &mut BehaviorContext<'_>) -> EngineResult {
        Ok(())
    }

    /// Called once, in entity handle order and then component order, before the
    /// first enable.
    fn initialize(&mut self, _context: &mut BehaviorContext<'_>) -> EngineResult {
        Ok(())
    }

    /// Called when the behaviour becomes active.
    fn enable(&mut self, _context: &mut BehaviorContext<'_>) -> EngineResult {
        Ok(())
    }

    /// Called when the behaviour stops being active.
    fn disable(&mut self, _context: &mut BehaviorContext<'_>) -> EngineResult {
        Ok(())
    }

    /// Called once per fixed step, before the physics steps.
    fn fixed_update(
        &mut self,
        _context: &mut BehaviorContext<'_>,
        _frame: &FixedFrameInfo,
    ) -> EngineResult {
        Ok(())
    }

    /// Called once per rendered frame.
    fn update(&mut self, _context: &mut BehaviorContext<'_>, _frame: &FrameInfo) -> EngineResult {
        Ok(())
    }

    /// Called once before the behaviour is destroyed. Its own components are
    /// still readable; references to other objects must be checked.
    fn destroy(&mut self, _context: &mut BehaviorContext<'_>) -> EngineResult {
        Ok(())
    }
}
