//! The scene runtime: it loads a scene asset, owns the behaviours of the scene
//! and performs every structural change, destruction, spawning and scene
//! replacement, at one boundary.
//!
//! The runtime does not own the scene: every call that drives behaviours takes
//! the [`Services`] it works on, so that a behaviour can be handed the scene and
//! the runtime together. Entity removal during play goes through
//! `Scene::queue_destroy_entity`, and [`Runtime::structural_boundary`] is the
//! single place where destruction, spawning and scene replacement happen.
//!
//! Modules:
//!
//! - `batch`: constructing, configuring and initializing the behaviours of one
//!   realization, and discarding them when it fails
//! - `drive`: calling behaviours, activation and the per-step updates
//! - `boundary`: loading and replacing scenes, spawning and destruction

mod batch;
mod boundary;
mod drive;

use fr_assets::AssetLibrary;
use fr_document::{AssetReference, Guid, PropertyOverride, SceneAsset, Severity};
use fr_scene::EntityHandle;
use fr_transform::Transform;

use crate::behavior::{Behavior, BehaviorId};
use crate::error::{EngineError, EngineResult};
use crate::registry::ComponentRegistry;

/// Identifies a queued spawn request; zero is none.
pub type SpawnToken = u64;

/// The progress of a spawn request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnState {
    /// Waiting for the next structural boundary.
    Pending,
    /// The prefab was realized.
    Succeeded,
    /// The prefab was not realized.
    Failed,
}

/// What became of a spawn request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnResult {
    /// The progress.
    pub state: SpawnState,
    /// The spawned root, when it succeeded; the null handle otherwise.
    pub root: EntityHandle,
    /// Why it failed.
    pub message: String,
}

/// A request to spawn a prefab at runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnRequest {
    /// The prefab asset.
    pub prefab: AssetReference,
    /// The name of the spawned root entity.
    pub name: String,
    /// The placement of the root relative to its parent.
    pub transform: Transform,
    /// The entity the root goes under; the null handle for the top level.
    pub parent: EntityHandle,
    /// The property overrides applied to the prefab's entities.
    pub overrides: Vec<PropertyOverride>,
}

impl Default for SpawnRequest {
    /// An unplaced prefab named "Prefab" under no parent, with no overrides.
    fn default() -> Self {
        Self {
            prefab: AssetReference::default(),
            name: "Prefab".to_owned(),
            transform: Transform::IDENTITY,
            parent: EntityHandle::NULL,
            overrides: Vec::new(),
        }
    }
}

/// One live behaviour with the component it was made from.
pub(crate) struct Slot {
    /// The id the runtime issued.
    pub(crate) id: BehaviorId,
    /// The entity the behaviour is on.
    pub(crate) entity: EntityHandle,
    /// The component the behaviour implements.
    pub(crate) component: Guid,
    /// The component type key.
    pub(crate) kind: String,
    /// Whether the behaviour asks to be enabled.
    pub(crate) enabled: bool,
    /// Whether the behaviour is enabled in the scene.
    pub(crate) active: bool,
    /// Whether the behaviour was initialized.
    pub(crate) initialized: bool,
    /// The behaviour; none while one of its callbacks runs.
    pub(crate) behavior: Option<Box<dyn Behavior>>,
}

/// Loads a scene and runs its behaviours, spawns and structural changes.
pub struct Runtime {
    /// The component types and behaviour factories.
    registry: ComponentRegistry,
    /// The behaviours in execution order, which is id order.
    slots: Vec<Slot>,
    /// The id the next behaviour gets.
    next_id: BehaviorId,
    /// The entities without a parent that the loaded scene and the spawns
    /// created, which an unload destroys.
    roots: Vec<EntityHandle>,
    /// The spawn requests that wait for the boundary.
    spawn_requests: Vec<(SpawnToken, SpawnRequest)>,
    /// What became of every request since the scene loaded.
    spawn_results: Vec<(SpawnToken, SpawnResult)>,
    /// The token the next request gets, less one.
    issued: SpawnToken,
    /// The scene that replaces the loaded one at the next boundary.
    pending_scene: Option<Box<SceneAsset>>,
    /// Whether a scene is loaded.
    loaded: bool,
    /// The scene loaded last, which a reload replaces the world with again.
    current_scene: Option<Box<SceneAsset>>,
    /// Messages about data that was kept but is inert.
    diagnostics: Vec<String>,
    /// The component types whose behaviours are paused.
    paused_kinds: Vec<String>,
}

impl Runtime {
    /// Creates a runtime with nothing loaded.
    pub fn new(registry: ComponentRegistry) -> Self {
        Self {
            registry,
            slots: Vec::new(),
            next_id: 1,
            roots: Vec::new(),
            spawn_requests: Vec::new(),
            spawn_results: Vec::new(),
            issued: 0,
            pending_scene: None,
            loaded: false,
            current_scene: None,
            diagnostics: Vec::new(),
            paused_kinds: Vec::new(),
        }
    }

    /// The component types and behaviour factories.
    pub const fn registry(&self) -> &ComponentRegistry {
        &self.registry
    }

    /// Whether a scene is loaded.
    pub const fn loaded(&self) -> bool {
        self.loaded
    }

    /// The messages of loading: component types that are unavailable and
    /// components that have no adapter.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// The number of live behaviours.
    pub fn behavior_count(&self) -> usize {
        self.slots.len()
    }

    /// Pauses or resumes every behaviour of a component type: while paused it
    /// gets no fixed update and no update, and nothing else changes.
    pub fn set_kind_enabled(&mut self, kind: &str, enabled: bool) {
        self.paused_kinds.retain(|paused| paused != kind);
        if !enabled {
            self.paused_kinds.push(kind.to_owned());
        }
    }

    /// Whether the behaviours of a component type run.
    pub fn kind_enabled(&self, kind: &str) -> bool {
        !self.paused_kinds.iter().any(|paused| paused == kind)
    }

    /// Queues a prefab spawn for the next structural boundary and answers the
    /// token to ask [`Runtime::spawn_result`] with. A newly spawned behaviour
    /// first updates on the following frame.
    pub fn spawn(&mut self, request: SpawnRequest) -> SpawnToken {
        self.issued += 1;
        let token = self.issued;
        self.spawn_results.push((
            token,
            SpawnResult {
                state: SpawnState::Pending,
                root: EntityHandle::NULL,
                message: String::new(),
            },
        ));
        self.spawn_requests.push((token, request));
        token
    }

    /// What became of a spawn request: pending, the spawned root or a failure
    /// message; a failure for a token never issued.
    pub fn spawn_result(&self, token: SpawnToken) -> SpawnResult {
        self.spawn_results
            .iter()
            .find(|(candidate, _)| *candidate == token)
            .map_or_else(
                || SpawnResult {
                    state: SpawnState::Failed,
                    root: EntityHandle::NULL,
                    message: "unknown spawn request".to_owned(),
                },
                |(_, result)| result.clone(),
            )
    }

    /// Validates a scene and keeps it to replace the loaded one at the next
    /// structural boundary. An invalid scene leaves the current one running.
    ///
    /// # Errors
    ///
    /// When the scene does not validate.
    pub fn queue_scene_asset(&mut self, asset: SceneAsset) -> EngineResult {
        let errors: Vec<String> =
            fr_document::validate_document(&asset.content, self.registry.schemas())
                .into_iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Error)
                .map(|diagnostic| diagnostic.message)
                .collect();
        if !errors.is_empty() {
            return Err(EngineError::new(format!(
                "the scene is invalid: {}",
                errors.join("; ")
            )));
        }
        self.pending_scene = Some(Box::new(asset));
        Ok(())
    }

    /// Queues the scene that was loaded last to replace the world at the next
    /// structural boundary, which puts every entity and behaviour back as the
    /// scene authored it.
    ///
    /// # Errors
    ///
    /// When no scene was loaded or it no longer validates.
    pub fn reload_scene(&mut self) -> EngineResult {
        let asset = self
            .current_scene
            .as_deref()
            .cloned()
            .ok_or_else(|| EngineError::new("no scene has been loaded"))?;
        self.queue_scene_asset(asset)
    }

    /// Reads the scene at a path, relative to the asset root, and queues it to
    /// replace the loaded one at the next structural boundary.
    ///
    /// # Errors
    ///
    /// When the scene cannot be read or does not validate.
    pub fn queue_scene(
        &mut self,
        assets: &mut AssetLibrary,
        path: &std::path::Path,
    ) -> EngineResult {
        let asset = assets.scene_at(path)?;
        self.queue_scene_asset((*asset).clone())
    }

    /// Tells the runtime that a behaviour asks to be enabled or disabled.
    pub(crate) fn set_enabled(&mut self, id: BehaviorId, enabled: bool) {
        if let Some(slot) = self.slot_mut(id) {
            slot.enabled = enabled;
        }
    }

    /// The position of a slot among the behaviours, which are ordered by id.
    fn position(&self, id: BehaviorId) -> Option<usize> {
        self.slots.binary_search_by_key(&id, |slot| slot.id).ok()
    }

    /// The slot of a behaviour, mutably.
    fn slot_mut(&mut self, id: BehaviorId) -> Option<&mut Slot> {
        let index = self.position(id)?;
        self.slots.get_mut(index)
    }

    /// The ids of every behaviour, in execution order.
    fn ids(&self) -> Vec<BehaviorId> {
        self.slots.iter().map(|slot| slot.id).collect()
    }
}
