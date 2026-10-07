//! The realization of authored documents into a stage: entities in parent-first
//! order, the prefab instances they nest, and the components of each created
//! through the registered adapters.
//!
//! One walk serves the scene a game starts in, a scene that replaces it and a
//! prefab spawned during play. Every entity a walk creates is tracked, so a
//! failure destroys exactly what that walk created and leaves the stage as it
//! was.

use std::collections::HashMap;

use fr_assets::AssetLibrary;
use fr_document::{
    ComponentRecord, ComponentSource, EntityDocument, EntityReference, Guid, NodeKind, PrefabAsset,
    PrefabInstanceRecord, PropertyOverride, SceneAsset, SchemaSet, Severity, effective_entity,
    validate_document,
};
use fr_transform::Transform;

use crate::adapters::RealizeContext;
use crate::error::SceneError;
use crate::hierarchy::{EntityConfig, EntityHandle};
use crate::stage::Stage;

/// Deepest nesting of prefab instances a walk follows.
const MAX_INSTANCE_DEPTH: usize = 32;

/// An entity a walk created, with the prefab instances above it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealizedEntity {
    /// The instances the entity is inside, outermost first; empty for an entity
    /// of the document itself.
    pub path: Vec<Guid>,
    /// The identity of the record the entity came from.
    pub id: Guid,
    /// The entity.
    pub handle: EntityHandle,
}

/// A component of a game type a walk found: the runtime constructs a behaviour
/// from it.
#[derive(Clone, Debug, PartialEq)]
pub struct RealizedBehaviour {
    /// The instances the entity is inside, outermost first.
    pub path: Vec<Guid>,
    /// The entity the component is on.
    pub entity: EntityHandle,
    /// The component record, effective after prefab overrides.
    pub record: ComponentRecord,
}

/// What one realization created.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Realization {
    /// The entities without a parent inside the realization, in order.
    pub roots: Vec<EntityHandle>,
    /// Every entity in creation order.
    pub entities: Vec<RealizedEntity>,
    /// The components of game types, in entity creation order then component
    /// order.
    pub behaviours: Vec<RealizedBehaviour>,
    /// Messages about data that is preserved but inert, such as a component
    /// type no registered schema describes.
    pub diagnostics: Vec<String>,
}

impl Realization {
    /// The entity an entity reference names, read from inside the prefab
    /// instances of `context`: the reference's own instance chain continues the
    /// context's.
    pub fn resolve_entity(
        &self,
        context: &[Guid],
        reference: &EntityReference,
    ) -> Option<EntityHandle> {
        if !reference.entity.valid() {
            return None;
        }
        let mut path = context.to_vec();
        path.extend_from_slice(&reference.instances);
        self.entities
            .iter()
            .find(|found| found.path == path && found.id == reference.entity)
            .map(|found| found.handle)
    }
}

/// Where a prefab's entities go: under a new entity named and placed by the
/// request.
#[derive(Clone, Debug)]
pub struct RootSpec {
    /// The name of the new root entity.
    pub name: String,
    /// The transform of the root relative to its parent.
    pub transform: Transform,
    /// The entity the root goes under; the null handle for the top level.
    pub parent: EntityHandle,
}

/// The state of one walk.
struct Walk<'a> {
    /// The stage being filled.
    stage: &'a mut Stage,
    /// The assets prefabs and models are read from.
    assets: &'a mut AssetLibrary,
    /// The component types known to the session.
    schemas: &'a SchemaSet,
    /// The entities created, in creation order.
    created: Vec<EntityHandle>,
    /// What the walk made so far.
    result: Realization,
}

impl Walk<'_> {
    /// Creates an entity and remembers it for rollback.
    fn create(&mut self, config: EntityConfig) -> EntityHandle {
        let handle = self.stage.create_entity(config);
        self.created.push(handle);
        handle
    }

    /// Records a created entity.
    fn record(&mut self, path: &[Guid], id: Guid, handle: EntityHandle) {
        self.result.entities.push(RealizedEntity {
            path: path.to_vec(),
            id,
            handle,
        });
    }

    /// Realizes the enabled components of an entity: engine types through their
    /// adapters in adapter order, game types collected as behaviours, and
    /// everything else reported as inert.
    fn components(
        &mut self,
        entity: EntityHandle,
        components: &[ComponentRecord],
        path: &[Guid],
    ) -> Result<(), SceneError> {
        let adapters = self.stage.adapters.clone();
        for adapter in &adapters {
            for record in components
                .iter()
                .filter(|record| record.enabled && record.kind == adapter.key())
            {
                let mut context = RealizeContext {
                    stage: &mut *self.stage,
                    assets: &mut *self.assets,
                    diagnostics: &mut self.result.diagnostics,
                };
                adapter.realize(&mut context, entity, record)?;
            }
        }
        for record in components.iter().filter(|record| record.enabled) {
            match self.schemas.find(&record.kind) {
                None => self.result.diagnostics.push(format!(
                    "component type {} is unavailable; its data is kept but inert",
                    record.kind
                )),
                Some(schema) if schema.source == ComponentSource::Game => {
                    self.result.behaviours.push(RealizedBehaviour {
                        path: path.to_vec(),
                        entity,
                        record: record.clone(),
                    });
                }
                Some(schema) if !adapters.iter().any(|adapter| adapter.key() == schema.key) => {
                    self.result.diagnostics.push(format!(
                        "component type {} has no adapter and is inert",
                        record.kind
                    ));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    /// Realizes the nodes of a document under a root, parents before children.
    fn build(
        &mut self,
        document: &EntityDocument,
        instance: Option<&PrefabInstanceRecord>,
        root: EntityHandle,
        path: &[Guid],
        depth: usize,
    ) -> Result<(), SceneError> {
        if depth > MAX_INSTANCE_DEPTH {
            return Err(SceneError::realization(
                "prefab instances nest deeper than the limit",
            ));
        }
        let mut known: HashMap<Guid, EntityHandle> = HashMap::new();
        for node in document.ordered_nodes() {
            let parent_id = document.node_parent(node.id);
            let parent = known.get(&parent_id).copied().unwrap_or(root);
            let handle = match node.kind {
                NodeKind::Entity => {
                    let Some(record) = document.find_entity(node.id) else {
                        continue;
                    };
                    let effective = effective_entity(record, instance);
                    let handle = self.create(EntityConfig {
                        id: effective.id,
                        name: effective.name.clone(),
                        transform: effective.transform,
                        parent,
                        active: effective.active,
                        groups: effective.groups.clone(),
                        components: effective.components.clone(),
                    });
                    self.record(path, node.id, handle);
                    self.components(handle, &effective.components, path)?;
                    handle
                }
                NodeKind::Instance => {
                    let Some(record) = document.find_instance(node.id) else {
                        continue;
                    };
                    let handle = self.create(EntityConfig {
                        id: record.id,
                        name: record.name.clone(),
                        transform: record.transform,
                        parent,
                        active: record.active,
                        groups: Vec::new(),
                        components: Vec::new(),
                    });
                    self.record(path, node.id, handle);
                    self.instance(record, handle, path, depth)?;
                    handle
                }
            };
            known.insert(node.id, handle);
            if self.stage.parent(handle) == Some(EntityHandle::NULL) {
                self.result.roots.push(handle);
            }
        }
        Ok(())
    }

    /// Realizes the prefab an instance names under the instance's entity.
    fn instance(
        &mut self,
        record: &PrefabInstanceRecord,
        entity: EntityHandle,
        path: &[Guid],
        depth: usize,
    ) -> Result<(), SceneError> {
        let prefab = self.assets.prefab(record.prefab.asset)?;
        check_document(&prefab.content, self.schemas)?;
        let mut inner = path.to_vec();
        inner.push(record.id);
        self.build(&prefab.content, Some(record), entity, &inner, depth + 1)
    }

    /// Destroys the entities the walk created, the latest first.
    fn undo(&mut self) {
        while let Some(handle) = self.created.pop() {
            if self.stage.contains(handle) {
                self.stage.destroy_entity(handle);
            }
        }
    }
}

/// Validates a document, failing on its errors and keeping nothing of its
/// warnings.
fn check_document(document: &EntityDocument, schemas: &SchemaSet) -> Result<(), SceneError> {
    let problems: Vec<String> = validate_document(document, schemas)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message)
        .collect();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(SceneError::Invalid { problems })
    }
}

impl Stage {
    /// Realizes a scene: validates it against `schemas`, applies its settings
    /// and creates its entities and components. The first camera that asks to
    /// be current becomes the current camera.
    ///
    /// # Errors
    ///
    /// [`SceneError::Invalid`] listing the errors of the document, or the
    /// failure of an asset or an adapter. Whatever was created is destroyed and
    /// the settings are restored.
    pub fn realize_scene(
        &mut self,
        assets: &mut AssetLibrary,
        schemas: &SchemaSet,
        scene: &SceneAsset,
    ) -> Result<Realization, SceneError> {
        check_document(&scene.content, schemas)?;
        let previous = self.settings;
        self.apply_settings(&scene.settings);
        let mut walk = Walk {
            stage: self,
            assets,
            schemas,
            created: Vec::new(),
            result: Realization::default(),
        };
        match walk.build(&scene.content, None, EntityHandle::NULL, &[], 0) {
            Ok(()) => {
                let result = walk.result;
                self.resolve_transforms();
                Ok(result)
            }
            Err(error) => {
                walk.undo();
                self.apply_settings(&previous);
                Err(error)
            }
        }
    }

    /// Realizes a prefab under a new root entity, with property overrides
    /// applied to the prefab's entities.
    ///
    /// # Errors
    ///
    /// [`SceneError::Invalid`] listing the errors of the prefab, or the failure
    /// of an asset or an adapter. Whatever was created is destroyed.
    pub fn realize_prefab(
        &mut self,
        assets: &mut AssetLibrary,
        schemas: &SchemaSet,
        prefab: &PrefabAsset,
        root: &RootSpec,
        overrides: &[PropertyOverride],
    ) -> Result<Realization, SceneError> {
        check_document(&prefab.content, schemas)?;
        let mut walk = Walk {
            stage: self,
            assets,
            schemas,
            created: Vec::new(),
            result: Realization::default(),
        };
        let handle = walk.create(EntityConfig {
            transform: root.transform,
            parent: root.parent,
            ..EntityConfig::new(root.name.clone())
        });
        walk.result.roots.push(handle);
        let placed = PrefabInstanceRecord {
            overrides: overrides.to_vec(),
            ..PrefabInstanceRecord::default()
        };
        match walk.build(&prefab.content, Some(&placed), handle, &[], 0) {
            Ok(()) => {
                let result = walk.result;
                self.resolve_transforms();
                Ok(result)
            }
            Err(error) => {
                walk.undo();
                Err(error)
            }
        }
    }
}
