//! Ordered transform propagation with validation of persistent parent references.

use fr_app::ecs as bevy_ecs;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fr_app::ecs::{entity::Entity, resource::Resource, world::World};

use crate::{GlobalTransform, LocalTransform, Parent, SceneId};

/// A hierarchy that cannot be propagated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HierarchyError(pub String);

impl fmt::Display for HierarchyError {
    /// Writes the hierarchy failure.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HierarchyError {}

/// The result of the most recent transform propagation.
#[derive(Resource, Clone, Debug)]
pub struct HierarchyStatus(pub Result<(), HierarchyError>);

impl Default for HierarchyStatus {
    /// Reports a valid empty hierarchy.
    fn default() -> Self {
        Self(Ok(()))
    }
}

/// Computes world matrices in entity order and rejects missing parents or cycles.
///
/// # Errors
///
/// Returns an error without changing world matrices when identifiers repeat,
/// a parent has no local transform, or the hierarchy contains a cycle.
pub fn propagate_transforms(world: &mut World) -> Result<(), HierarchyError> {
    let mut query = world.query::<(Entity, &LocalTransform, Option<&SceneId>, Option<&Parent>)>();
    let mut ids = BTreeMap::new();
    let mut nodes = BTreeMap::new();
    for (entity, local, id, parent) in query.iter(world) {
        if let Some(id) = id
            && ids.insert(id.clone(), entity).is_some()
        {
            return Err(HierarchyError(format!(
                "duplicate object identifier {}",
                id.0
            )));
        }
        nodes.insert(
            entity,
            (local.transform().matrix(), parent.map(|p| p.0.clone())),
        );
    }
    let mut children: BTreeMap<Entity, Vec<Entity>> = BTreeMap::new();
    let mut ready = BTreeSet::new();
    for (&entity, (_, parent)) in &nodes {
        if let Some(parent) = parent {
            let parent_entity = ids
                .get(parent)
                .ok_or_else(|| HierarchyError(format!("missing parent transform {}", parent.0)))?;
            children.entry(*parent_entity).or_default().push(entity);
        } else {
            ready.insert(entity);
        }
    }
    let mut matrices = BTreeMap::new();
    while let Some(entity) = ready.pop_first() {
        let (local, parent) = &nodes[&entity];
        let matrix = match parent {
            Some(parent) => matrices[&ids[parent]] * *local,
            None => *local,
        };
        matrices.insert(entity, matrix);
        if let Some(children) = children.get(&entity) {
            ready.extend(children.iter().copied());
        }
    }
    if matrices.len() != nodes.len() {
        return Err(HierarchyError(String::from(
            "the object hierarchy contains a cycle",
        )));
    }
    for (entity, matrix) in matrices {
        world.entity_mut(entity).insert(GlobalTransform(matrix));
    }
    let mut stale = world.query_filtered::<Entity, (
        fr_app::ecs::query::With<GlobalTransform>,
        fr_app::ecs::query::Without<LocalTransform>,
    )>();
    let stale: Vec<_> = stale.iter(world).collect();
    for entity in stale {
        world.entity_mut(entity).remove::<GlobalTransform>();
    }
    Ok(())
}

/// Records propagation failures for the host or an editor to inspect.
pub(crate) fn update_transforms(world: &mut World) {
    let result = propagate_transforms(world);
    world.resource_mut::<HierarchyStatus>().0 = result;
}
