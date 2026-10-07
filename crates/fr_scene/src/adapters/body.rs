//! The rigid body and collider adapters.

use std::sync::Arc;

use fr_document::ComponentRecord;
use fr_document::builtin_schema::{COLLIDER, RIGID_BODY};
use fr_physics::body::{BodyDef, BodyId, BodyType};
use fr_physics::geometry::{Capsule, Geometry, Sphere};
use fr_physics::hull::Hull;
use fr_physics::math::Pose;
use fr_physics::shape::{Material, ShapeDef};

use super::{ComponentAdapter, RealizeContext, props_of};
use crate::error::SceneError;
use crate::hierarchy::EntityHandle;
use crate::stage::Stage;

/// The live state of a rigid body component: the body it created.
#[derive(Clone, Copy, Debug)]
pub struct BodyState {
    /// The body in the stage's physics world.
    pub body: BodyId,
    /// How the body moves.
    pub body_type: BodyType,
}

/// Realizes `forge.rigid_body`: a physics body at its entity's placement. Its
/// mass comes from the densities of the colliders attached to it.
pub struct RigidBodyAdapter;

impl ComponentAdapter for RigidBodyAdapter {
    /// The rigid body type key.
    fn key(&self) -> &'static str {
        RIGID_BODY
    }

    /// Creates the body at the entity's resolved transform.
    fn realize(
        &self,
        context: &mut RealizeContext<'_>,
        entity: EntityHandle,
        record: &ComponentRecord,
    ) -> Result<(), SceneError> {
        let props = props_of(context.stage, record)?;
        let body_type = match props.text("body_type") {
            "static" => BodyType::Static,
            "kinematic" => BodyType::Kinematic,
            _ => BodyType::Dynamic,
        };
        let definition = BodyDef {
            linear_damping: props.float("linear_damping"),
            angular_damping: props.float("angular_damping"),
            gravity_scale: props.float("gravity_scale"),
            ..BodyDef::new(body_type)
        };
        let stage = &mut *context.stage;
        let global = stage
            .global_transform(entity)
            .ok_or(SceneError::MissingEntity)?;
        let body = stage.world.create_body(&BodyDef {
            position: global.translation,
            rotation: global.rotation,
            ..definition
        });
        stage.bodies.insert(entity, BodyState { body, body_type });
        Ok(())
    }

    /// Destroys the body with every shape on it.
    fn release(&self, stage: &mut Stage, entity: EntityHandle) {
        if let Some(state) = stage.bodies.remove(&entity) {
            stage.world.destroy_body(state.body);
        }
    }
}

/// Realizes `forge.collider`: a shape on the rigid body of the same entity.
pub struct ColliderAdapter;

impl ComponentAdapter for ColliderAdapter {
    /// The collider type key.
    fn key(&self) -> &'static str {
        COLLIDER
    }

    /// Attaches the shape to the entity's body.
    fn realize(
        &self,
        context: &mut RealizeContext<'_>,
        entity: EntityHandle,
        record: &ComponentRecord,
    ) -> Result<(), SceneError> {
        let props = props_of(context.stage, record)?;
        let offset = props.vec3("offset");
        let radius = props.float("radius");
        let geometry = match props.text("shape") {
            "sphere" => Geometry::Sphere(Sphere {
                center: offset,
                radius,
            }),
            "capsule" => {
                let half = props.float("half_height");
                Geometry::Capsule(Capsule::new(
                    offset - fr_math::Vec3::new(0.0, half, 0.0),
                    offset + fr_math::Vec3::new(0.0, half, 0.0),
                    radius,
                ))
            }
            _ => Geometry::Hull(Arc::new(
                Hull::cuboid(props.vec3("half_extents")).transformed(&Pose::from_position(offset)),
            )),
        };
        let material = Material {
            friction: props.float("friction"),
            restitution: props.float("restitution"),
            rolling_resistance: props.float("rolling_resistance"),
            ..Material::default()
        };
        let mut definition = ShapeDef::new(geometry)
            .with_density(props.float("density"))
            .with_material(material);
        if props.boolean("sensor") {
            definition = definition.as_sensor();
        }
        let stage = &mut *context.stage;
        let state = stage
            .bodies
            .get(&entity)
            .copied()
            .ok_or_else(|| SceneError::realization("a collider needs a rigid body"))?;
        stage
            .world
            .create_shape(state.body, &definition)
            .ok_or_else(|| SceneError::realization("the collider shape is not valid"))?;
        Ok(())
    }

    /// Nothing: the body owns its shapes.
    fn release(&self, _stage: &mut Stage, _entity: EntityHandle) {}
}
