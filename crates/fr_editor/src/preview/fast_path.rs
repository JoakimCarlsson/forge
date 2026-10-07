//! Recognising an edit that only moved nodes, which the preview applies without
//! rebuilding the stage.

use fr_authoring::DocumentData;
use fr_document::Guid;
use fr_transform::Transform;

/// The new transform of every node that differs between two states of a
/// document, when nothing else differs: the same nodes, in the same order, with
/// the same names, components, parents and settings. None when any other edit
/// happened, which needs a rebuild.
pub fn changed_transforms(
    old: &DocumentData,
    new: &DocumentData,
) -> Option<Vec<(Guid, Transform)>> {
    if old.settings != new.settings
        || old.content.entities.len() != new.content.entities.len()
        || old.content.instances.len() != new.content.instances.len()
    {
        return None;
    }
    let mut changes = Vec::new();
    for (before, after) in old.content.entities.iter().zip(&new.content.entities) {
        if before == after {
            continue;
        }
        let mut probe = before.clone();
        probe.transform = after.transform;
        if probe != *after {
            return None;
        }
        changes.push((after.id, after.transform));
    }
    for (before, after) in old.content.instances.iter().zip(&new.content.instances) {
        if before == after {
            continue;
        }
        let mut probe = before.clone();
        probe.transform = after.transform;
        if probe != *after {
            return None;
        }
        changes.push((after.id, after.transform));
    }
    Some(changes)
}
