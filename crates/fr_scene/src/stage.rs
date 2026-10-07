//! The live scene: the entity hierarchy, the component stores their adapters
//! fill, and the physics world.

use std::collections::BTreeMap;
use std::rc::Rc;

use fr_document::{AmbientSettings, Color, ComponentRecord, Guid, SceneSettings, SchemaSet};
use fr_physics::body::BodyId;
use fr_physics::world::{Stats, World, WorldDef};
use fr_transform::Transform;

use crate::adapters::{
    BodyState, CameraState, ComponentAdapter, LightState, RagdollState, RendererState,
    builtin_adapters,
};
use crate::error::SceneError;
use crate::hierarchy::{EntityConfig, EntityHandle, Hierarchy};

/// The live scene.
///
/// A stage owns the hierarchy of entities, each with the records of its
/// components, and the live state the registered adapters created from them:
/// cameras, lights, renderers and the physics world with its bodies. It is
/// headless: nothing in it needs a window or a graphics device, and the frame
/// it describes reaches the graphics side through
/// [`crate::RenderResources`].
///
/// Entities are created and destroyed at one boundary per frame. Removal during
/// play goes through [`Stage::queue_destroy_entity`] and
/// [`Stage::flush_destroy_queue`]; [`Stage::destroy_entity`] is only for setup
/// and teardown.
pub struct Stage {
    /// The entities.
    pub(crate) hierarchy: Hierarchy,
    /// The physics world.
    pub(crate) world: World,
    /// The settings of the scene last applied.
    pub(crate) settings: SceneSettings,
    /// Whether lights may cast shadows at all.
    pub(crate) shadows_enabled: bool,
    /// The adapters in realization order.
    pub(crate) adapters: Vec<Rc<dyn ComponentAdapter>>,
    /// The built-in component types, whose defaults adapters read.
    pub(crate) defaults: SchemaSet,
    /// The cameras by entity.
    pub(crate) cameras: BTreeMap<EntityHandle, CameraState>,
    /// The camera the frame is seen through.
    pub(crate) current_camera: EntityHandle,
    /// The lights by entity.
    pub(crate) lights: BTreeMap<EntityHandle, LightState>,
    /// The mesh renderers by entity.
    pub(crate) renderers: BTreeMap<EntityHandle, RendererState>,
    /// The rigid bodies by entity.
    pub(crate) bodies: BTreeMap<EntityHandle, BodyState>,
    /// The ragdolls by entity.
    pub(crate) ragdolls: BTreeMap<EntityHandle, RagdollState>,
    /// How far the drawn bodies are between their last two steps.
    pub(crate) interpolation: f32,
    /// The collision group of the latest ragdoll.
    pub(crate) next_ragdoll_group: i32,
    /// The entities queued for destruction at the next boundary.
    destroy_queue: Vec<EntityHandle>,
}

impl Default for Stage {
    /// An empty stage with the built-in adapters.
    fn default() -> Self {
        Self::new()
    }
}

impl Stage {
    /// An empty stage with the built-in adapters and the default settings.
    pub fn new() -> Self {
        let settings = SceneSettings::default();
        Self {
            hierarchy: Hierarchy::default(),
            world: World::new(world_definition(&settings)),
            settings,
            shadows_enabled: true,
            adapters: builtin_adapters(),
            defaults: fr_document::builtin_schemas(),
            cameras: BTreeMap::new(),
            current_camera: EntityHandle::NULL,
            lights: BTreeMap::new(),
            renderers: BTreeMap::new(),
            bodies: BTreeMap::new(),
            ragdolls: BTreeMap::new(),
            interpolation: 1.0,
            next_ragdoll_group: 0,
            destroy_queue: Vec::new(),
        }
    }

    /// Applies the settings of a scene: the ambient light, the background and
    /// the physics. A stage with no bodies gets a fresh physics world, so a
    /// scene replaced by another starts from step zero.
    pub fn apply_settings(&mut self, settings: &SceneSettings) {
        self.settings = *settings;
        if self.world.bodies().next().is_none() {
            self.world = World::new(world_definition(settings));
            self.next_ragdoll_group = 0;
        } else {
            self.world.set_gravity(settings.physics.gravity);
        }
    }

    /// The settings of the scene last applied.
    pub fn settings(&self) -> &SceneSettings {
        &self.settings
    }

    /// The colour the frame is cleared to.
    pub fn clear_color(&self) -> Color {
        self.settings.clear_color
    }

    /// The ambient light.
    pub fn ambient(&self) -> &AmbientSettings {
        &self.settings.ambient
    }

    /// Lets lights cast shadows, or none at all whatever they ask for.
    pub fn set_shadows_enabled(&mut self, enabled: bool) {
        self.shadows_enabled = enabled;
    }

    /// Whether lights may cast shadows.
    pub fn shadows_enabled(&self) -> bool {
        self.shadows_enabled
    }

    /// The component types the stage realizes itself, with their defaults.
    pub fn builtin_schemas(&self) -> &SchemaSet {
        &self.defaults
    }

    /// Creates an entity. Realization and setup code uses it; a game spawns
    /// prefabs through the runtime instead.
    pub fn create_entity(&mut self, config: EntityConfig) -> EntityHandle {
        self.hierarchy.create(config)
    }

    /// Destroys an entity and everything below it immediately, releasing what
    /// the adapters made, and answers the handles destroyed. Only setup and
    /// teardown call it: during play, removal is queued.
    pub fn destroy_entity(&mut self, entity: EntityHandle) -> Vec<EntityHandle> {
        let adapters = self.adapters.clone();
        let doomed = self.hierarchy.subtree(entity);
        for handle in doomed.iter().rev() {
            for adapter in &adapters {
                adapter.release(self, *handle);
            }
        }
        self.queue_forget(&doomed);
        self.hierarchy.remove_subtree(entity)
    }

    /// Removes destroyed handles from the destroy queue.
    fn queue_forget(&mut self, gone: &[EntityHandle]) {
        self.destroy_queue.retain(|queued| !gone.contains(queued));
    }

    /// Records an entity to destroy at the next structural boundary and changes
    /// nothing else, so handles stay valid for the rest of the frame.
    pub fn queue_destroy_entity(&mut self, entity: EntityHandle) {
        if self.hierarchy.contains(entity) && !self.destroy_queue.contains(&entity) {
            self.destroy_queue.push(entity);
        }
    }

    /// Every entity the next flush destroys: the queued ones and everything
    /// below them, each once.
    pub fn pending_destruction(&self) -> Vec<EntityHandle> {
        let mut doomed: Vec<EntityHandle> = Vec::new();
        for queued in &self.destroy_queue {
            for handle in self.hierarchy.subtree(*queued) {
                if !doomed.contains(&handle) {
                    doomed.push(handle);
                }
            }
        }
        doomed
    }

    /// Destroys the queued entities with everything below them: the one place
    /// in a frame where queued destruction happens. Answers the handles
    /// destroyed.
    pub fn flush_destroy_queue(&mut self) -> Vec<EntityHandle> {
        let queued = std::mem::take(&mut self.destroy_queue);
        let mut destroyed = Vec::new();
        for entity in queued {
            if self.hierarchy.contains(entity) {
                destroyed.extend(self.destroy_entity(entity));
            }
        }
        destroyed
    }

    /// Whether a handle names a live entity.
    pub fn contains(&self, entity: EntityHandle) -> bool {
        self.hierarchy.contains(entity)
    }

    /// The number of live entities.
    pub fn entity_count(&self) -> usize {
        self.hierarchy.len()
    }

    /// Every live entity in handle order.
    pub fn entities(&self) -> impl Iterator<Item = EntityHandle> + '_ {
        self.hierarchy.handles()
    }

    /// The entities without a parent, in creation order.
    pub fn roots(&self) -> &[EntityHandle] {
        self.hierarchy.roots()
    }

    /// The name of an entity.
    pub fn name(&self, entity: EntityHandle) -> Option<&str> {
        self.hierarchy.get(entity).map(|found| found.name.as_str())
    }

    /// The authored identity of an entity; unset for one made at runtime. An
    /// entity of a prefab shares the identity of the prefab's record with every
    /// other instance.
    pub fn authored_id(&self, entity: EntityHandle) -> Option<Guid> {
        self.hierarchy.get(entity).map(|found| found.id)
    }

    /// The parent of an entity; the null handle for a root.
    pub fn parent(&self, entity: EntityHandle) -> Option<EntityHandle> {
        self.hierarchy.get(entity).map(|found| found.parent)
    }

    /// The children of an entity in order.
    pub fn children(&self, entity: EntityHandle) -> &[EntityHandle] {
        self.hierarchy
            .get(entity)
            .map_or(&[], |found| found.children.as_slice())
    }

    /// Moves an entity under a parent, or to the roots with the null handle,
    /// keeping its local transform. Returns whether it moved.
    pub fn set_parent(&mut self, entity: EntityHandle, parent: EntityHandle) -> bool {
        self.hierarchy.set_parent(entity, parent)
    }

    /// Whether an entity is in a group.
    pub fn in_group(&self, entity: EntityHandle, group: &str) -> bool {
        self.hierarchy
            .get(entity)
            .is_some_and(|found| found.groups.iter().any(|name| name == group))
    }

    /// The entities in a group, in handle order.
    pub fn entities_in_group(&self, group: &str) -> Vec<EntityHandle> {
        self.hierarchy
            .handles()
            .filter(|handle| self.in_group(*handle, group))
            .collect()
    }

    /// The first entity with a name, in handle order.
    pub fn find_by_name(&self, name: &str) -> Option<EntityHandle> {
        self.hierarchy
            .handles()
            .find(|handle| self.name(*handle) == Some(name))
    }

    /// The records of an entity's components, effective after prefab overrides.
    pub fn components(&self, entity: EntityHandle) -> &[ComponentRecord] {
        self.hierarchy
            .get(entity)
            .map_or(&[], |found| found.components.as_slice())
    }

    /// The first component of a type on an entity.
    pub fn find_component(&self, entity: EntityHandle, kind: &str) -> Option<&ComponentRecord> {
        self.components(entity)
            .iter()
            .find(|component| component.kind == kind)
    }

    /// Whether an entity asks to be active.
    pub fn is_active(&self, entity: EntityHandle) -> bool {
        self.hierarchy.get(entity).is_some_and(|found| found.active)
    }

    /// Whether an entity and every ancestor ask to be active. An inactive
    /// entity is neither drawn nor given behaviour updates.
    pub fn active_in_hierarchy(&self, entity: EntityHandle) -> bool {
        self.hierarchy.active_in_hierarchy(entity)
    }

    /// Activates or deactivates an entity and, through it, everything below.
    pub fn set_active(&mut self, entity: EntityHandle, active: bool) {
        if let Some(found) = self.hierarchy.get_mut(entity) {
            found.active = active;
        }
    }

    /// The transform of an entity relative to its parent.
    pub fn transform(&self, entity: EntityHandle) -> Option<Transform> {
        self.hierarchy.get(entity).map(|found| found.local)
    }

    /// Sets the transform of an entity relative to its parent. The entity and
    /// its descendants are only marked stale: anything that reads a global pose
    /// resolves first.
    pub fn set_transform(&mut self, entity: EntityHandle, transform: Transform) {
        self.hierarchy.set_local(entity, transform);
    }

    /// The transform of an entity in the world, resolved first.
    pub fn global_transform(&mut self, entity: EntityHandle) -> Option<Transform> {
        self.hierarchy.resolve_transforms();
        self.hierarchy.get(entity).map(|found| found.global)
    }

    /// Recomputes the global transform of every entity that changed. The frame
    /// loop calls it after the structural boundary and before the frame is
    /// drawn; physics and every accessor that hands out a global pose resolve
    /// first, and resolving twice costs nothing.
    pub fn resolve_transforms(&mut self) {
        self.hierarchy.resolve_transforms();
    }

    /// The camera the frame is seen through, when there is one.
    pub fn current_camera(&self) -> Option<EntityHandle> {
        Some(self.current_camera).filter(|camera| self.cameras.contains_key(camera))
    }

    /// Makes the camera of an entity the one the frame is seen through.
    ///
    /// # Errors
    ///
    /// [`SceneError::MissingEntity`] when the entity has no camera.
    pub fn set_current_camera(&mut self, entity: EntityHandle) -> Result<(), SceneError> {
        if !self.cameras.contains_key(&entity) {
            return Err(SceneError::MissingEntity);
        }
        self.current_camera = entity;
        Ok(())
    }

    /// The physics body of an entity, when it has a rigid body.
    pub fn body_of(&self, entity: EntityHandle) -> Option<BodyId> {
        self.bodies.get(&entity).map(|state| state.body)
    }

    /// The physics world, to read.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// The physics world, to act on bodies directly. Creating or destroying
    /// bodies behind the stage's back leaves entities without them.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// The counts of the physics world.
    pub fn physics_stats(&self) -> Stats {
        self.world.stats()
    }
}

/// The physics world settings of a scene's settings.
fn world_definition(settings: &SceneSettings) -> WorldDef {
    WorldDef {
        gravity: settings.physics.gravity,
        ..WorldDef::default()
    }
}
