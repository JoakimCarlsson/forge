//! The light adapter.

use fr_document::ComponentRecord;
use fr_document::builtin_schema::LIGHT;
use fr_math::Vec3;

use super::{ComponentAdapter, RealizeContext, props_of};
use crate::error::SceneError;
use crate::hierarchy::EntityHandle;
use crate::stage::Stage;

/// Which kind of light a light component is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    /// Parallel light from infinitely far away.
    Directional,
    /// Light in every direction from a point.
    Point,
    /// Light in a cone from a point.
    Spot,
}

/// The live state of a light component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightState {
    /// The kind of light.
    pub kind: LightKind,
    /// The linear colour.
    pub color: Vec3,
    /// The intensity in the units of `fr_light`.
    pub intensity: f32,
    /// The distance beyond which a point or spot light fades out.
    pub range: f32,
    /// The half-angle in radians inside which a spot light is at full strength.
    pub inner_angle: f32,
    /// The half-angle in radians beyond which a spot light gives no light.
    pub outer_angle: f32,
    /// Whether objects block the light.
    pub cast_shadows: bool,
    /// How far from the camera a directional light's shadows reach.
    pub shadow_distance: f32,
}

/// Realizes `forge.light`: a directional or spot light shines along its entity's
/// forward axis, negative Z, and a point or spot light sits at its entity.
pub struct LightAdapter;

impl ComponentAdapter for LightAdapter {
    /// The light type key.
    fn key(&self) -> &'static str {
        LIGHT
    }

    /// Stores the light.
    fn realize(
        &self,
        context: &mut RealizeContext<'_>,
        entity: EntityHandle,
        record: &ComponentRecord,
    ) -> Result<(), SceneError> {
        let props = props_of(context.stage, record)?;
        let kind = match props.text("kind") {
            "directional" => LightKind::Directional,
            "spot" => LightKind::Spot,
            _ => LightKind::Point,
        };
        let state = LightState {
            kind,
            color: props.color("color").to_vec3(),
            intensity: props.float("intensity"),
            range: props.float("range"),
            inner_angle: props.float("inner_angle_degrees").to_radians(),
            outer_angle: props.float("outer_angle_degrees").to_radians(),
            cast_shadows: props.boolean("cast_shadows"),
            shadow_distance: props.float("shadow_distance"),
        };
        context.stage.lights.insert(entity, state);
        Ok(())
    }

    /// Drops the light.
    fn release(&self, stage: &mut Stage, entity: EntityHandle) {
        stage.lights.remove(&entity);
    }
}
