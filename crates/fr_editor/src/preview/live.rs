//! The live stage of the document being authored.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use fr_assets::AssetLibrary;
use fr_authoring::{Document, DocumentData};
use fr_camera::Camera;
use fr_color::Rgba;
use fr_document::Guid;
use fr_document::builtin_schema::{RAGDOLL, RIGID_BODY};
use fr_math::Aabb;
use fr_render::{Renderer, Scene};
use fr_scene::{EntityHandle, GpuResources, RealizedEntity, ResourceCache, Stage};
use fr_transform::Transform;

use super::bounds::{NodeBounds, OrientedBox};
use super::build::build_stage;
use super::fast_path::changed_transforms;
use super::frame::headlight;
use crate::subsystems::{BoundsContext, SubsystemRegistry};

/// How many times larger than the first quartile of nodes a node may be and
/// still count toward the box a camera frames.
const OUTSIZE_FACTOR: f32 = 8.0;

/// The smallest first quartile, in world units, the framing limit is taken
/// from, so a scene of markers does not exclude everything with an extent.
const MINIMUM_QUARTILE: f32 = 0.5;

/// Which state of which document a stage was built from. Revisions grow on
/// every change, undo and redo included, so equal keys mean equal content.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DocumentKey {
    /// The file of the document.
    path: PathBuf,
    /// The document's revision.
    revision: u64,
}

impl DocumentKey {
    /// The key of a document as it is now.
    fn of(document: &Document) -> Self {
        Self {
            path: document.path().to_path_buf(),
            revision: document.revision(),
        }
    }
}

/// An authoring [`Document`] turned into a live [`Stage`] through the engine's
/// registered adapters, so what the editor shows is what a game realizes.
///
/// Nothing simulates: the stage is never stepped, so physics and ragdoll
/// components are inert data and a dynamic body stays where it was placed. The
/// stage is rebuilt when the document's identity or revision changes, except
/// that an edit which only moved nodes is applied with `set_transform`. A
/// failed realization keeps the previous stage and reports its message.
///
/// The preview maps authored node identities to entity handles and back (an
/// entity inside a prefab instance maps to the instance's node, the outermost
/// one, which is what picking and selection address), keeps the world
/// transform of every node of the document and the world bounds the
/// subsystems contribute for picking and framing.
pub struct Preview {
    /// The assets models and prefabs are read from.
    assets: AssetLibrary,
    /// What is on the device for the frames built so far.
    cache: ResourceCache,
    /// The live scene.
    stage: Stage,
    /// The entities the last realization created.
    entities: Vec<RealizedEntity>,
    /// The entity of each node of the document, instance nodes included.
    handles: HashMap<Guid, EntityHandle>,
    /// The node each entity answers to: its own, or the outermost instance
    /// above it.
    nodes: HashMap<EntityHandle, Guid>,
    /// The world transform of every node of the document.
    world: HashMap<Guid, Transform>,
    /// The bounds of every pickable node.
    bounds: Vec<NodeBounds>,
    /// The position of each node's bounds in `bounds`.
    bounds_index: HashMap<Guid, usize>,
    /// The document state the stage reflects or last tried to reflect.
    key: Option<DocumentKey>,
    /// The content the stage actually reflects, which an edit is compared with.
    stage_data: Option<DocumentData>,
    /// The messages of the last realization.
    diagnostics: Vec<String>,
}

impl Preview {
    /// An empty preview reading assets from a project's asset root.
    pub fn new(asset_root: &Path) -> Self {
        Self {
            assets: AssetLibrary::open(asset_root),
            cache: ResourceCache::default(),
            stage: Stage::new(),
            entities: Vec::new(),
            handles: HashMap::new(),
            nodes: HashMap::new(),
            world: HashMap::new(),
            bounds: Vec::new(),
            bounds_index: HashMap::new(),
            key: None,
            stage_data: None,
            diagnostics: Vec::new(),
        }
    }

    /// Forgets the stage, for when no document is open.
    pub fn clear(&mut self) {
        self.stage = Stage::new();
        self.entities.clear();
        self.handles.clear();
        self.nodes.clear();
        self.world.clear();
        self.bounds.clear();
        self.bounds_index.clear();
        self.key = None;
        self.stage_data = None;
        self.diagnostics.clear();
    }

    /// Reads the asset library again from disk and forces the next
    /// [`Preview::sync`] to rebuild, after models or prefabs changed.
    pub fn reload_assets(&mut self) {
        let root = self.assets.root().to_path_buf();
        self.assets = AssetLibrary::open(&root);
        self.key = None;
    }

    /// Brings the stage up to date with a document and returns the messages of
    /// the work done: nothing when the stage already reflects the document, the
    /// realization's diagnostics after a rebuild, or the failure that kept the
    /// previous stage.
    pub fn sync(&mut self, document: &Document, registry: &SubsystemRegistry) -> Vec<String> {
        let key = DocumentKey::of(document);
        if self.key.as_ref() == Some(&key) {
            return Vec::new();
        }
        let same_file = self
            .key
            .as_ref()
            .is_some_and(|previous| previous.path == key.path);
        self.key = Some(key);
        if same_file && self.apply_moves(document) {
            self.refresh(document, registry);
            return Vec::new();
        }
        match build_stage(&mut self.assets, document) {
            Ok(built) => {
                self.stage = built.stage;
                self.install(built.entities);
                self.stage_data = Some(document.data().clone());
                self.refresh(document, registry);
                self.diagnostics = built.diagnostics.clone();
                built.diagnostics
            }
            Err(message) => {
                self.diagnostics = vec![message.clone()];
                vec![message]
            }
        }
    }

    /// Applies an edit that only moved nodes to the stage; false when the edit
    /// was anything else or moves something a simulation owns, which a rebuild
    /// handles.
    fn apply_moves(&mut self, document: &Document) -> bool {
        let Some(previous) = self.stage_data.as_ref() else {
            return false;
        };
        let Some(changes) = changed_transforms(previous, document.data()) else {
            return false;
        };
        let mut targets = Vec::with_capacity(changes.len());
        for (node, transform) in changes {
            let Some(handle) = self.handles.get(&node).copied() else {
                return false;
            };
            if self.is_simulated(handle) {
                return false;
            }
            targets.push((handle, transform));
        }
        for (handle, transform) in targets {
            self.stage.set_transform(handle, transform);
        }
        self.stage_data = Some(document.data().clone());
        true
    }

    /// Whether an entity or anything under it holds a body or a ragdoll, whose
    /// pose lives in the physics world rather than in its transform.
    fn is_simulated(&self, entity: EntityHandle) -> bool {
        let own = self.stage.body_of(entity).is_some()
            || self
                .stage
                .components(entity)
                .iter()
                .any(|component| component.kind == RAGDOLL || component.kind == RIGID_BODY);
        own || self
            .stage
            .children(entity)
            .iter()
            .any(|child| self.is_simulated(*child))
    }

    /// Takes the identity maps from a realization's entities.
    fn install(&mut self, entities: Vec<RealizedEntity>) {
        self.handles.clear();
        self.nodes.clear();
        for entity in &entities {
            let node = entity.path.first().copied().unwrap_or(entity.id);
            self.nodes.insert(entity.handle, node);
            if entity.path.is_empty() {
                self.handles.insert(entity.id, entity.handle);
            }
        }
        self.entities = entities;
    }

    /// Recomputes what derives from the stage: the resolved transforms, the
    /// drawn poses and the world transforms and bounds of the nodes. The drawn
    /// poses of a stage that never stepped are its placements, and
    /// `apply_transform_interpolation` is the only writer of the poses a frame
    /// is described from, so it runs here at its final position.
    fn refresh(&mut self, document: &Document, registry: &SubsystemRegistry) {
        self.stage.resolve_transforms();
        self.stage.apply_transform_interpolation(1.0);
        self.world.clear();
        let mut globals = Vec::with_capacity(self.entities.len());
        for entity in &self.entities {
            let global = self
                .stage
                .global_transform(entity.handle)
                .unwrap_or(Transform::IDENTITY);
            if entity.path.is_empty() {
                self.world.insert(entity.id, global);
            }
            globals.push(global);
        }
        self.refresh_bounds(document, registry, &globals);
    }

    /// Recomputes the bounds of every node from the contributions of the
    /// components in the stage, and gives each node of the document that has no
    /// extent a small marker box.
    fn refresh_bounds(
        &mut self,
        document: &Document,
        registry: &SubsystemRegistry,
        globals: &[Transform],
    ) {
        let mut context = BoundsContext::new(&mut self.assets);
        let mut parts: Vec<(Guid, Vec<OrientedBox>)> = Vec::new();
        let mut index: HashMap<Guid, usize> = HashMap::new();
        for (entity, global) in self.entities.iter().zip(globals) {
            if !self.stage.active_in_hierarchy(entity.handle) {
                continue;
            }
            let node = entity.path.first().copied().unwrap_or(entity.id);
            for component in self
                .stage
                .components(entity.handle)
                .iter()
                .filter(|component| component.enabled)
            {
                let Some(local) = registry.bounds(component, document.schemas(), &mut context)
                else {
                    continue;
                };
                let slot = *index.entry(node).or_insert_with(|| {
                    parts.push((node, Vec::new()));
                    parts.len() - 1
                });
                parts[slot].1.push(OrientedBox {
                    local,
                    transform: *global,
                });
            }
        }
        let mut bounds: Vec<NodeBounds> = parts
            .into_iter()
            .filter_map(|(node, boxes)| NodeBounds::from_parts(node, boxes))
            .collect();
        let measured: HashSet<Guid> = bounds.iter().map(|found| found.node).collect();
        for node in document.content().ordered_nodes() {
            let active = self
                .handles
                .get(&node.id)
                .is_some_and(|handle| self.stage.active_in_hierarchy(*handle));
            if measured.contains(&node.id) || !active {
                continue;
            }
            if let Some(transform) = self.world.get(&node.id) {
                bounds.push(NodeBounds::marker(node.id, *transform));
            }
        }
        self.bounds_index = bounds
            .iter()
            .enumerate()
            .map(|(position, found)| (found.node, position))
            .collect();
        self.bounds = bounds;
    }

    /// Describes the stage as one frame: the scene lights and instances, seen
    /// through the editor camera instead of the scene's own camera entity,
    /// which is only an authored object here. When the document has no active
    /// light a directional headlight is added over the camera's shoulder so the
    /// viewport is never black.
    ///
    /// # Errors
    ///
    /// The message of the resource that could not be provided; `scene` is then
    /// incomplete and the previous frame should be kept.
    pub fn build_frame(
        &mut self,
        renderer: &mut Renderer,
        camera: &Camera,
        scene: &mut Scene,
    ) -> Result<(), String> {
        let mut resources = GpuResources {
            renderer,
            assets: &mut self.assets,
            cache: &mut self.cache,
        };
        self.stage
            .build_render_scene(&mut resources, scene)
            .map_err(|error| error.to_string())?;
        scene.camera = *camera;
        if scene.lights.is_empty() {
            scene.add_light(headlight(camera));
        }
        Ok(())
    }

    /// The colour the frame is cleared to, as sRGB.
    pub fn clear_color(&self) -> Rgba {
        let color = self.stage.clear_color();
        Rgba::new(color.r, color.g, color.b, 1.0)
    }

    /// The messages of the last realization or failure.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// The stage, for reading.
    pub fn stage(&self) -> &Stage {
        &self.stage
    }

    /// The asset library the preview reads models and prefabs from.
    pub fn assets(&self) -> &AssetLibrary {
        &self.assets
    }

    /// The authored node an entity answers to: its own, or the outermost prefab
    /// instance above it.
    pub fn node_of(&self, entity: EntityHandle) -> Option<Guid> {
        self.nodes.get(&entity).copied()
    }

    /// The entity of a node of the document.
    pub fn entity_of(&self, node: Guid) -> Option<EntityHandle> {
        self.handles.get(&node).copied()
    }

    /// The world transform of a node of the document.
    pub fn world_transform(&self, node: Guid) -> Option<Transform> {
        self.world.get(&node).copied()
    }

    /// The bounds of every pickable node.
    pub fn bounds(&self) -> &[NodeBounds] {
        &self.bounds
    }

    /// The bounds of one node.
    pub fn node_bounds(&self, node: Guid) -> Option<&NodeBounds> {
        self.bounds_index
            .get(&node)
            .and_then(|position| self.bounds.get(*position))
    }

    /// The box around every node's bounds, which a camera frames.
    pub fn scene_bounds(&self) -> Option<Aabb> {
        self.bounds
            .iter()
            .map(|found| found.world)
            .reduce(|a, b| a.union(&b))
    }

    /// The box a camera frames on opening a document: the box around the
    /// nodes of ordinary size. A ground plane or a backdrop many times larger
    /// than most of what the scene holds would otherwise push the camera so far
    /// away that the content is a speck, so a node whose diagonal is more than
    /// [`OUTSIZE_FACTOR`] times that of the first quartile of nodes is left
    /// out, unless that leaves nothing.
    pub fn framing_bounds(&self) -> Option<Aabb> {
        let mut diagonals: Vec<f32> = self
            .bounds
            .iter()
            .map(|found| found.world.extents().length() * 2.0)
            .collect();
        diagonals.sort_by(f32::total_cmp);
        let quartile = diagonals.get(diagonals.len() / 4).copied().unwrap_or(0.0);
        let limit = quartile.max(MINIMUM_QUARTILE) * OUTSIZE_FACTOR;
        self.bounds
            .iter()
            .filter(|found| found.world.extents().length() * 2.0 <= limit)
            .map(|found| found.world)
            .reduce(|a, b| a.union(&b))
            .or_else(|| self.scene_bounds())
    }

    /// The box around the bounds of some nodes.
    pub fn bounds_of(&self, nodes: &[Guid]) -> Option<Aabb> {
        nodes
            .iter()
            .filter_map(|node| self.node_bounds(*node))
            .map(|found| found.world)
            .reduce(|a, b| a.union(&b))
    }
}
