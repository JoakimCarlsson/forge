//! The entity hierarchy: stable handles, parents and ordered children, local
//! transforms with their resolved globals, and the records of each entity's
//! components.

use fr_document::{ComponentRecord, Guid};
use fr_physics::slot::{Handle, SlotMap};
use fr_transform::Transform;

/// Marks [`EntityHandle`].
#[derive(Clone, Copy, Debug)]
pub struct EntityTag;

/// A stable, generational handle to an entity: a handle from an unloaded scene,
/// or to a destroyed entity, never resolves again.
pub type EntityHandle = Handle<EntityTag>;

/// How a new entity starts.
#[derive(Clone, Debug)]
pub struct EntityConfig {
    /// The authored identity; unset for an entity made at runtime.
    pub id: Guid,
    /// The display name.
    pub name: String,
    /// The transform relative to the parent.
    pub transform: Transform,
    /// The parent; the null handle for a root.
    pub parent: EntityHandle,
    /// Whether the entity starts active.
    pub active: bool,
    /// The groups the entity belongs to.
    pub groups: Vec<String>,
    /// The records of its components, effective after prefab overrides.
    pub components: Vec<ComponentRecord>,
}

impl EntityConfig {
    /// An active root entity with an identity transform and no components.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Guid::NONE,
            name: name.into(),
            transform: Transform::IDENTITY,
            parent: EntityHandle::NULL,
            active: true,
            groups: Vec::new(),
            components: Vec::new(),
        }
    }
}

/// One entity: its place in the hierarchy, its transforms and its records.
#[derive(Clone, Debug)]
pub(crate) struct Entity {
    /// The authored identity, unset for an entity made at runtime.
    pub(crate) id: Guid,
    /// The display name.
    pub(crate) name: String,
    /// The transform relative to the parent.
    pub(crate) local: Transform,
    /// The transform in the world as of the last resolve.
    pub(crate) global: Transform,
    /// The transform drawn this frame: the global with the interpolation of
    /// simulated bodies applied. Only rendering reads it.
    pub(crate) render: Transform,
    /// The parent; the null handle for a root.
    pub(crate) parent: EntityHandle,
    /// The children in order.
    pub(crate) children: Vec<EntityHandle>,
    /// Whether the entity asks to be active.
    pub(crate) active: bool,
    /// The groups the entity belongs to.
    pub(crate) groups: Vec<String>,
    /// The records of its components.
    pub(crate) components: Vec<ComponentRecord>,
    /// Whether the global is stale.
    pub(crate) dirty: bool,
}

/// The entities of a scene.
#[derive(Default)]
pub struct Hierarchy {
    /// The entities by handle.
    entities: SlotMap<Entity, EntityTag>,
    /// The entities without a parent, in creation order.
    roots: Vec<EntityHandle>,
    /// Whether any entity's global is stale.
    any_dirty: bool,
}

impl Hierarchy {
    /// Creates an entity. A parent that is not live makes the entity a root.
    pub fn create(&mut self, config: EntityConfig) -> EntityHandle {
        let parent = if self.contains(config.parent) {
            config.parent
        } else {
            EntityHandle::NULL
        };
        let handle = self.entities.insert(Entity {
            id: config.id,
            name: config.name,
            local: config.transform,
            global: config.transform,
            render: config.transform,
            parent,
            children: Vec::new(),
            active: config.active,
            groups: config.groups,
            components: config.components,
            dirty: true,
        });
        match self.entities.get_mut(parent) {
            Some(owner) => owner.children.push(handle),
            None => self.roots.push(handle),
        }
        self.any_dirty = true;
        handle
    }

    /// Whether a handle names a live entity.
    pub fn contains(&self, handle: EntityHandle) -> bool {
        self.entities.contains(handle)
    }

    /// The number of live entities.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Whether no entity is live.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Every live entity in handle order.
    pub fn handles(&self) -> impl Iterator<Item = EntityHandle> + '_ {
        self.entities.handles()
    }

    /// The entity of a handle.
    pub(crate) fn get(&self, handle: EntityHandle) -> Option<&Entity> {
        self.entities.get(handle)
    }

    /// The entity of a handle, mutably.
    pub(crate) fn get_mut(&mut self, handle: EntityHandle) -> Option<&mut Entity> {
        self.entities.get_mut(handle)
    }

    /// The entities without a parent, in creation order.
    pub fn roots(&self) -> &[EntityHandle] {
        &self.roots
    }

    /// The handle, with its descendants after it, depth first.
    pub fn subtree(&self, handle: EntityHandle) -> Vec<EntityHandle> {
        let mut out = Vec::new();
        self.collect_subtree(handle, &mut out);
        out
    }

    /// Appends an entity and everything below it.
    fn collect_subtree(&self, handle: EntityHandle, out: &mut Vec<EntityHandle>) {
        let Some(entity) = self.entities.get(handle) else {
            return;
        };
        out.push(handle);
        for child in &entity.children {
            self.collect_subtree(*child, out);
        }
    }

    /// Removes an entity and everything below it, and answers the handles
    /// removed, descendants before their parents.
    pub fn remove_subtree(&mut self, handle: EntityHandle) -> Vec<EntityHandle> {
        let mut removed = self.subtree(handle);
        removed.reverse();
        let parent = self
            .entities
            .get(handle)
            .map_or(EntityHandle::NULL, |entity| entity.parent);
        match self.entities.get_mut(parent) {
            Some(owner) => owner.children.retain(|child| *child != handle),
            None => self.roots.retain(|root| *root != handle),
        }
        for gone in &removed {
            self.entities.remove(*gone);
        }
        removed
    }

    /// Moves an entity under a parent, or to the roots with the null handle,
    /// keeping its local transform. An entity cannot go under itself or one of
    /// its descendants.
    ///
    /// Returns whether the entity moved.
    pub fn set_parent(&mut self, handle: EntityHandle, parent: EntityHandle) -> bool {
        if !self.contains(handle) || (!parent.is_null() && !self.contains(parent)) {
            return false;
        }
        if self.subtree(handle).contains(&parent) {
            return false;
        }
        let old = self
            .entities
            .get(handle)
            .map_or(EntityHandle::NULL, |entity| entity.parent);
        match self.entities.get_mut(old) {
            Some(owner) => owner.children.retain(|child| *child != handle),
            None => self.roots.retain(|root| *root != handle),
        }
        match self.entities.get_mut(parent) {
            Some(owner) => owner.children.push(handle),
            None => self.roots.push(handle),
        }
        if let Some(entity) = self.entities.get_mut(handle) {
            entity.parent = parent;
        }
        self.mark_dirty(handle);
        true
    }

    /// Sets the transform of an entity relative to its parent and marks it and
    /// its descendants stale.
    pub fn set_local(&mut self, handle: EntityHandle, transform: Transform) {
        if let Some(entity) = self.entities.get_mut(handle) {
            entity.local = transform;
            self.mark_dirty(handle);
        }
    }

    /// Marks an entity and everything below it stale.
    fn mark_dirty(&mut self, handle: EntityHandle) {
        if let Some(entity) = self.entities.get_mut(handle) {
            entity.dirty = true;
            self.any_dirty = true;
        }
    }

    /// Places an entity at a transform in the world by setting the local
    /// transform that produces it under the resolved parent. The entity's own
    /// global is current afterwards and its descendants are stale.
    pub(crate) fn set_global(&mut self, handle: EntityHandle, global: Transform) {
        let Some(parent) = self.entities.get(handle).map(|entity| entity.parent) else {
            return;
        };
        let local = match self.entities.get(parent) {
            Some(owner) => {
                Transform::from_matrix(owner.global.matrix().inverse() * global.matrix())
            }
            None => global,
        };
        let children = match self.entities.get_mut(handle) {
            Some(entity) => {
                entity.local = local;
                entity.global = global;
                entity.dirty = false;
                entity.children.clone()
            }
            None => return,
        };
        for child in children {
            self.mark_dirty(child);
        }
    }

    /// Recomputes the global transform of every stale entity, parents before
    /// children. Nothing is done when nothing is stale, so resolving twice
    /// costs nothing.
    pub fn resolve_transforms(&mut self) {
        if !self.any_dirty {
            return;
        }
        let roots = self.roots.clone();
        for root in roots {
            self.resolve_node(root, None, false);
        }
        self.any_dirty = false;
    }

    /// Resolves an entity and, when it or an ancestor was stale, its children.
    fn resolve_node(&mut self, handle: EntityHandle, parent: Option<Transform>, forced: bool) {
        let Some(entity) = self.entities.get_mut(handle) else {
            return;
        };
        let stale = forced || entity.dirty;
        if stale {
            entity.global = parent.map_or(entity.local, |above| entity.local.then(&above));
            entity.dirty = false;
        }
        let (global, children) = (entity.global, entity.children.clone());
        for child in children {
            self.resolve_node(child, Some(global), stale);
        }
    }

    /// Sets the drawn transform of every entity: `driven` answers the transform
    /// of an entity a simulation moves, and every other entity follows its
    /// parent. Call it after [`Hierarchy::resolve_transforms`].
    pub(crate) fn compute_render_transforms(
        &mut self,
        driven: &dyn Fn(EntityHandle) -> Option<Transform>,
    ) {
        let roots = self.roots.clone();
        for root in roots {
            self.render_node(root, None, driven);
        }
    }

    /// Sets the drawn transform of an entity and everything below it.
    fn render_node(
        &mut self,
        handle: EntityHandle,
        parent: Option<Transform>,
        driven: &dyn Fn(EntityHandle) -> Option<Transform>,
    ) {
        let Some(entity) = self.entities.get_mut(handle) else {
            return;
        };
        entity.render = match driven(handle) {
            Some(transform) => transform,
            None => parent.map_or(entity.global, |above| entity.local.then(&above)),
        };
        let (render, children) = (entity.render, entity.children.clone());
        for child in children {
            self.render_node(child, Some(render), driven);
        }
    }

    /// Whether an entity and every ancestor ask to be active.
    pub fn active_in_hierarchy(&self, handle: EntityHandle) -> bool {
        let mut current = handle;
        while let Some(entity) = self.entities.get(current) {
            if !entity.active {
                return false;
            }
            current = entity.parent;
        }
        self.contains(handle)
    }
}
