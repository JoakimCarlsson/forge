//! Describing the stage as one frame of `fr_render`'s scene.

use fr_camera::Camera;
use fr_light::{DirectionalLight, Light, PointLight, SpotLight};
use fr_math::{Quat, Vec3};
use fr_physics::geometry::Geometry;
use fr_physics::math::Pose;
use fr_render::{AmbientLight, Scene};
use fr_transform::Transform;

use crate::adapters::{LightKind, LightState, RagdollState, RendererSource, RendererState};
use crate::error::SceneError;
use crate::resources::{Primitive, RenderResources};
use crate::stage::Stage;

/// The diameter of a bone marker sphere.
const BONE_MARKER_SIZE: f32 = 0.05;

/// The direction an entity faces: its forward axis, negative Z.
fn forward_of(transform: &Transform) -> Vec3 {
    transform.rotation * Vec3::NEG_Z
}

impl Stage {
    /// Describes the stage as one frame: the camera, the ambient light, the
    /// lights and every drawn instance, placed by the transforms
    /// [`Stage::apply_transform_interpolation`] last set. `scene` is cleared
    /// first and keeps its previous camera when the stage has none. Nothing in
    /// the stage changes, so the frame loop calls it after everything that can
    /// mutate the scene.
    ///
    /// # Errors
    ///
    /// The failure of `resources` to provide a model, mesh or material.
    pub fn build_render_scene(
        &self,
        resources: &mut dyn RenderResources,
        scene: &mut Scene,
    ) -> Result<(), SceneError> {
        scene.clear();
        let ambient = self.ambient();
        scene.ambient = AmbientLight {
            sky_color: ambient.sky_color,
            ground_color: ambient.ground_color,
            intensity: ambient.intensity,
        };
        if let Some(camera) = self.render_camera() {
            scene.camera = camera;
        }
        for (entity, light) in &self.lights {
            if self.active_in_hierarchy(*entity)
                && let Some(found) = self.hierarchy.get(*entity)
            {
                scene.add_light(self.render_light(light, &found.render));
            }
        }
        for (entity, renderer) in &self.renderers {
            if self.active_in_hierarchy(*entity)
                && let Some(found) = self.hierarchy.get(*entity)
            {
                draw_renderer(resources, scene, renderer, &found.render)?;
            }
        }
        for (entity, ragdoll) in &self.ragdolls {
            if self.active_in_hierarchy(*entity) {
                self.draw_ragdoll(resources, scene, ragdoll)?;
            }
        }
        Ok(())
    }

    /// The camera of the current camera entity at its drawn transform.
    fn render_camera(&self) -> Option<Camera> {
        let entity = self.current_camera()?;
        if !self.active_in_hierarchy(entity) {
            return None;
        }
        let state = self.cameras.get(&entity)?;
        let transform = self.hierarchy.get(entity)?.render;
        Some(Camera {
            position: transform.translation,
            target: transform.translation + forward_of(&transform),
            up: transform.rotation * Vec3::Y,
            projection: state.projection,
            exposure: state.exposure,
        })
    }

    /// A light at its drawn transform.
    fn render_light(&self, light: &LightState, transform: &Transform) -> Light {
        let cast_shadows = light.cast_shadows && self.shadows_enabled;
        match light.kind {
            LightKind::Directional => Light::Directional(DirectionalLight {
                direction: forward_of(transform),
                color: light.color,
                intensity: light.intensity,
                cast_shadows,
                shadow_distance: light.shadow_distance,
            }),
            LightKind::Point => Light::Point(PointLight {
                position: transform.translation,
                color: light.color,
                intensity: light.intensity,
                range: light.range,
                cast_shadows,
            }),
            LightKind::Spot => Light::Spot(SpotLight {
                position: transform.translation,
                direction: forward_of(transform),
                color: light.color,
                intensity: light.intensity,
                range: light.range,
                inner_angle: light.inner_angle,
                outer_angle: light.outer_angle,
                cast_shadows,
            }),
        }
    }

    /// Draws the shapes of every body of a ragdoll at their interpolated poses,
    /// and a marker at each bone.
    fn draw_ragdoll(
        &self,
        resources: &mut dyn RenderResources,
        scene: &mut Scene,
        state: &RagdollState,
    ) -> Result<(), SceneError> {
        let limbs = resources.material(&state.limbs)?;
        for body in state.ragdoll.bodies() {
            let Some(pose) = self.world.body_interpolated_pose(body, self.interpolation) else {
                continue;
            };
            for shape in self.world.body_shapes(body) {
                let Some(geometry) = self.world.shape_geometry(shape) else {
                    continue;
                };
                let (primitive, local, scale) = shape_primitive(geometry);
                let mesh = resources.primitive(primitive)?;
                scene.add(
                    mesh,
                    limbs,
                    pose.mul(&local).to_transform().with_scale(scale),
                );
            }
        }
        if let Some(bones) = &state.bones {
            let material = resources.material(bones)?;
            let marker = resources.primitive(Primitive::Sphere)?;
            for bone in state
                .ragdoll
                .bone_interpolated_poses(&self.world, self.interpolation)
            {
                scene.add(
                    marker,
                    material,
                    bone.with_scale(Vec3::splat(BONE_MARKER_SIZE)),
                );
            }
        }
        Ok(())
    }
}

/// Adds what a mesh renderer draws at a transform.
fn draw_renderer(
    resources: &mut dyn RenderResources,
    scene: &mut Scene,
    renderer: &RendererState,
    transform: &Transform,
) -> Result<(), SceneError> {
    let material = renderer
        .material
        .as_ref()
        .map(|spec| resources.material(spec))
        .transpose()?;
    match renderer.source {
        RendererSource::Model(id) => {
            let model = resources.model(id)?;
            match material {
                None => scene.add_model(&model, *transform),
                Some(material) => {
                    for instance in &model.instances {
                        scene.add(instance.mesh, material, instance.transform.then(transform));
                    }
                }
            }
        }
        RendererSource::Primitive(primitive) => {
            let mesh = resources.primitive(primitive)?;
            scene.add(
                mesh,
                material.unwrap_or_else(|| resources.default_material()),
                *transform,
            );
        }
    }
    Ok(())
}

/// The primitive that draws a collision shape, with the shape's pose in its
/// body and the scale that fits the primitive to it.
fn shape_primitive(geometry: &Geometry) -> (Primitive, Pose, Vec3) {
    match geometry {
        Geometry::Sphere(ball) => (
            Primitive::Sphere,
            Pose::from_position(ball.center),
            Vec3::splat(ball.radius * 2.0),
        ),
        Geometry::Capsule(pill) => {
            let axis = pill.center2() - pill.center1();
            let length = axis.length();
            let rotation = if length > 1e-6 {
                Quat::from_rotation_arc(Vec3::Y, axis / length)
            } else {
                Quat::IDENTITY
            };
            (
                Primitive::capsule(pill.radius, length),
                Pose::new((pill.center1() + pill.center2()) * 0.5, rotation),
                Vec3::ONE,
            )
        }
        Geometry::Hull(hull) => {
            let bounds = hull.aabb();
            (
                Primitive::Cube,
                Pose::from_position(bounds.center()),
                bounds.extents() * 2.0,
            )
        }
    }
}
