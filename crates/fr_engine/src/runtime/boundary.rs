//! Loading and replacing scenes, spawning prefabs and destroying entities: the
//! structural changes of a frame, all made at one boundary.

use fr_document::SceneAsset;
use fr_scene::{EntityHandle, Realization, RootSpec};

use super::{Runtime, SpawnRequest, SpawnResult, SpawnState, SpawnToken};
use crate::error::{EngineError, EngineResult};
use crate::services::Services;

impl Runtime {
    /// Replaces the loaded scene with the asset: unloads the current one,
    /// validates and realizes the new one and constructs and initializes its
    /// behaviours. On failure everything created is destroyed and the runtime
    /// is left with no scene loaded.
    ///
    /// # Errors
    ///
    /// The reason the scene did not load.
    pub fn load(&mut self, services: &mut Services<'_>, asset: &SceneAsset) -> EngineResult {
        self.unload(services)?;
        let built =
            services
                .scene
                .realize_scene(services.assets, self.registry.schemas(), asset)?;
        self.diagnostics.clone_from(&built.diagnostics);
        if let Err(error) = self.adopt(services, &built) {
            self.destroy_roots(services, &built.roots);
            return Err(error);
        }
        self.roots = built.roots;
        self.current_scene = Some(Box::new(asset.clone()));
        self.loaded = true;
        Ok(())
    }

    /// Tears down the behaviours, destroys the realized and spawned entities and
    /// forgets pending spawns.
    ///
    /// # Errors
    ///
    /// The first failure of a behaviour while it is torn down.
    pub fn unload(&mut self, services: &mut Services<'_>) -> EngineResult {
        let outcome = self.teardown(services);
        let roots = std::mem::take(&mut self.roots);
        self.destroy_roots(services, &roots);
        self.spawn_requests.clear();
        self.spawn_results.clear();
        self.diagnostics.clear();
        self.loaded = false;
        outcome
    }

    /// Destroys the entities under roots that are still alive.
    fn destroy_roots(&mut self, services: &mut Services<'_>, roots: &[EntityHandle]) {
        for root in roots {
            if services.scene.contains(*root) {
                services.scene.destroy_entity(*root);
            }
        }
    }

    /// Records what became of a spawn request.
    fn settle(&mut self, token: SpawnToken, state: SpawnState, root: EntityHandle, message: &str) {
        if let Some((_, result)) = self
            .spawn_results
            .iter_mut()
            .find(|(candidate, _)| *candidate == token)
        {
            *result = SpawnResult {
                state,
                root,
                message: message.to_owned(),
            };
        }
    }

    /// Replaces the scene when one is queued; otherwise tears down the
    /// behaviours of dying entities, flushes the destroy queue, then realizes
    /// the queued spawns. Call it after the user interface and before the
    /// transforms resolve. Destruction requested from a callback waits for the
    /// next boundary and is never flushed recursively.
    ///
    /// # Errors
    ///
    /// The failure of a behaviour or of a scene replacement.
    pub fn structural_boundary(&mut self, services: &mut Services<'_>) -> EngineResult {
        if let Some(asset) = self.pending_scene.take() {
            return self.load(services, &asset);
        }
        let dying = services.scene.pending_destruction();
        let torn_down = self.drop_behaviors_on(services, &dying);
        services.scene.flush_destroy_queue();
        self.roots.retain(|root| services.scene.contains(*root));
        torn_down?;
        for (token, request) in std::mem::take(&mut self.spawn_requests) {
            match self.spawn_one(services, &request) {
                Ok(root) => self.settle(token, SpawnState::Succeeded, root, ""),
                Err(error) => self.settle(
                    token,
                    SpawnState::Failed,
                    EntityHandle::NULL,
                    error.message(),
                ),
            }
        }
        Ok(())
    }

    /// Realizes one spawn request and constructs its behaviours.
    fn spawn_one(
        &mut self,
        services: &mut Services<'_>,
        request: &SpawnRequest,
    ) -> Result<EntityHandle, EngineError> {
        let prefab = services.assets.prefab(request.prefab.asset)?;
        let parent = if services.scene.contains(request.parent) {
            request.parent
        } else {
            EntityHandle::NULL
        };
        let root = RootSpec {
            name: request.name.clone(),
            transform: request.transform,
            parent,
        };
        let built: Realization = services.scene.realize_prefab(
            services.assets,
            self.registry.schemas(),
            &prefab,
            &root,
            &request.overrides,
        )?;
        if let Err(error) = self.adopt(services, &built) {
            self.destroy_roots(services, &built.roots);
            return Err(error);
        }
        let spawned = built.roots.first().copied().unwrap_or(EntityHandle::NULL);
        self.roots.extend(built.roots);
        Ok(spawned)
    }
}
