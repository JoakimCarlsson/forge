//! Synchronization of logical CPU asset handles with renderer-owned GPU handles.

use std::collections::BTreeMap;

use fr_image::TextureId;
use fr_material::MaterialId;
use fr_mesh::MeshId;
use fr_render::{RenderError, Renderer, Scene};

use crate::Assets;
use crate::loader::AssetEntry;

/// A GPU handle and the CPU revision that produced it.
struct Uploaded<T> {
    /// The renderer's handle.
    id: T,
    /// The uploaded CPU revision.
    revision: u64,
}

/// GPU representations of the app's currently owned CPU assets.
#[derive(Default)]
pub(crate) struct Uploads {
    /// Logical to GPU mesh handles.
    meshes: BTreeMap<MeshId, Uploaded<MeshId>>,
    /// Logical to GPU material handles.
    materials: BTreeMap<MaterialId, Uploaded<MaterialId>>,
    /// Logical to GPU texture handles.
    textures: BTreeMap<TextureId, Uploaded<TextureId>>,
}

impl Uploads {
    /// Uploads changed assets and releases removed or replaced GPU resources.
    pub(crate) fn sync(
        &mut self,
        assets: &Assets,
        renderer: &mut Renderer,
    ) -> Result<(), RenderError> {
        let textures_changed = sync_entries(
            &assets.textures,
            &mut self.textures,
            renderer,
            false,
            |texture, renderer| {
                renderer.create_texture(&texture.image, texture.sampler, texture.srgb)
            },
            Renderer::remove_texture,
        )?;
        sync_entries(
            &assets.meshes,
            &mut self.meshes,
            renderer,
            false,
            |mesh, renderer| renderer.create_mesh(mesh),
            Renderer::remove_mesh,
        )?;
        sync_entries(
            &assets.materials,
            &mut self.materials,
            renderer,
            textures_changed,
            |material, renderer| {
                let material = material.try_map_textures(|id, _| {
                    self.textures
                        .get(id)
                        .map(|uploaded| uploaded.id)
                        .ok_or_else(|| {
                            RenderError::InvalidAsset(String::from(
                                "a material refers to an unloaded texture",
                            ))
                        })
                })?;
                renderer.create_material(&material)
            },
            Renderer::remove_material,
        )?;
        Ok(())
    }

    /// Converts frame instances to GPU handles, omitting released assets.
    pub(crate) fn resolve(&self, scene: &Scene) -> Scene {
        Scene {
            camera: scene.camera,
            lights: scene.lights.clone(),
            ambient: scene.ambient,
            instances: scene
                .instances
                .iter()
                .filter_map(|instance| {
                    Some(fr_render::MeshInstance {
                        mesh: self.meshes.get(&instance.mesh)?.id,
                        material: self.materials.get(&instance.material)?.id,
                        transform: instance.transform,
                    })
                })
                .collect(),
        }
    }
}

/// Synchronizes one asset table and reports whether its GPU representations changed.
fn sync_entries<K: Copy + Ord, T>(
    entries: &BTreeMap<K, AssetEntry<T>>,
    uploaded: &mut BTreeMap<K, Uploaded<K>>,
    renderer: &mut Renderer,
    force: bool,
    mut create: impl FnMut(&T, &mut Renderer) -> Result<K, RenderError>,
    remove: fn(&mut Renderer, K),
) -> Result<bool, RenderError> {
    let mut changed = false;
    uploaded.retain(|id, uploaded| {
        if entries.contains_key(id) {
            return true;
        }
        remove(renderer, uploaded.id);
        changed = true;
        false
    });
    for (&id, entry) in entries {
        if !force
            && uploaded
                .get(&id)
                .is_some_and(|uploaded| uploaded.revision == entry.revision)
        {
            continue;
        }
        let gpu = create(&entry.data, renderer)?;
        if let Some(previous) = uploaded.insert(
            id,
            Uploaded {
                id: gpu,
                revision: entry.revision,
            },
        ) {
            remove(renderer, previous.id);
        }
        changed = true;
    }
    Ok(changed)
}
