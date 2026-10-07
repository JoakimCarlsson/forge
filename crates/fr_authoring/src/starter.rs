//! The content of files the Project panel creates.

use fr_document::components::{make_component, make_entity, set_property};
use fr_document::{
    Color, EntityDocument, EntityRecord, PrefabAsset, PropertyValue, SceneAsset, SceneSettings,
    builtin_schema, builtin_schemas,
};
use fr_math::{Quat, Vec3};
use fr_transform::Transform;

/// An entity with a component of a built-in type whose properties are set.
fn entity_with(
    name: &str,
    transform: Transform,
    kind: &str,
    properties: &[(&str, PropertyValue)],
) -> EntityRecord {
    let schemas = builtin_schemas();
    let mut entity = make_entity(name);
    entity.transform = transform;
    if let Some(schema) = schemas.find(kind) {
        let mut component = make_component(schema);
        for (key, value) in properties {
            set_property(&mut component, key, value.clone());
        }
        entity.components.push(component);
    }
    entity
}

/// A scene with a camera and a directional sun, the same as the first scene of
/// a new project.
pub fn starter_scene() -> SceneAsset {
    let camera = entity_with(
        "Camera",
        Transform::from_translation(Vec3::new(0.0, 2.0, 6.0)),
        builtin_schema::CAMERA,
        &[],
    );
    let direction = Vec3::new(0.4, -1.0, -0.3).normalize();
    let sun = entity_with(
        "Sun",
        Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, direction)),
        builtin_schema::LIGHT,
        &[
            ("kind", PropertyValue::Enum("directional".to_owned())),
            ("intensity", PropertyValue::Float(3.0)),
            ("cast_shadows", PropertyValue::Bool(true)),
            ("color", PropertyValue::Color(Color::rgb(1.0, 1.0, 1.0))),
        ],
    );
    SceneAsset {
        content: EntityDocument {
            entities: vec![camera, sun],
            instances: Vec::new(),
        },
        settings: SceneSettings::default(),
        ..SceneAsset::default()
    }
}

/// A scene with no entities and the default settings.
pub fn empty_scene() -> SceneAsset {
    SceneAsset::default()
}

/// A prefab with one root entity of a name.
pub fn starter_prefab(name: &str) -> PrefabAsset {
    PrefabAsset {
        content: EntityDocument {
            entities: vec![make_entity(name)],
            instances: Vec::new(),
        },
        ..PrefabAsset::default()
    }
}
