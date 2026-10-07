//! Scaffolding a new game project from the templates in
//! `crates/fr_project/templates`.

use std::path::{Path, PathBuf};

use fr_document::components::{make_component, make_entity, set_property};
use fr_document::{
    Color, EntityDocument, EntityRecord, PropertyValue, SceneAsset, SceneSettings, builtin_schemas,
    ensure_asset_meta, write_file, write_scene_text,
};
use fr_math::{Quat, Vec3};
use fr_transform::Transform;

use crate::error::ProjectError;
use crate::project::Project;
use crate::settings::ProjectSettings;

/// The manifest of a new project.
const MANIFEST_TEMPLATE: &str = include_str!("../templates/Cargo.toml.in");

/// The entry point of a new game.
const MAIN_TEMPLATE: &str = include_str!("../templates/main.rs.in");

/// The behaviour module of a new game.
const BEHAVIOURS_TEMPLATE: &str = include_str!("../templates/behaviours.rs.in");

/// The ignore file of a new project.
const IGNORE_TEMPLATE: &str = include_str!("../templates/gitignore.in");

/// The asset root of a new project, relative to its directory.
const ASSET_ROOT: &str = "assets";

/// The scene a new project starts in, relative to its asset root.
const STARTUP_SCENE: &str = "scenes/main.scene";

/// The directory of the `fr_engine` crate that a game of this checkout depends
/// on.
pub fn engine_crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fr_engine")
}

/// A name as a valid Cargo package name: lower case letters, digits and
/// underscores, with a `p` prefix when it is empty or starts with a digit.
pub fn package_name(name: &str) -> String {
    let mut value: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if value
        .chars()
        .next()
        .is_none_or(|first| first.is_ascii_digit())
    {
        value.insert(0, 'p');
    }
    value
}

/// A path as text with forward slashes, as a manifest takes it.
fn generic(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}

/// A text as the contents of a TOML basic string.
fn toml_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The `Cargo.toml` of a new project.
fn manifest(package: &str, engine_dir: &Path) -> String {
    MANIFEST_TEMPLATE
        .replace("@NAME@", package)
        .replace("@ENGINE@", &toml_string(&generic(engine_dir)))
}

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

/// The first scene of a new project: a camera and a sun.
fn starter_scene() -> SceneAsset {
    let camera = entity_with(
        "Camera",
        Transform::from_translation(Vec3::new(0.0, 2.0, 6.0)),
        fr_document::builtin_schema::CAMERA,
        &[],
    );
    let direction = Vec3::new(0.4, -1.0, -0.3).normalize();
    let sun = entity_with(
        "Sun",
        Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, direction)),
        fr_document::builtin_schema::LIGHT,
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

/// Creates a new project in a directory from the project templates: a
/// `Cargo.toml` depending on the `fr_engine` crate at `engine_dir`, a game
/// entry point and behaviour module, `project.forge`, an ignore file and a
/// first scene with its sidecar under `assets/`.
///
/// # Errors
///
/// [`ProjectError::AlreadyExists`] when the directory already holds a project,
/// or the failure of the first file that cannot be written.
pub fn create_project(root: &Path, engine_dir: &Path) -> Result<Project, ProjectError> {
    if Project::exists_in(root) {
        return Err(ProjectError::AlreadyExists {
            root: root.to_path_buf(),
        });
    }
    let name = root
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let package = package_name(&name);
    let manifest_text = manifest(&package, engine_dir);
    let files = [
        ("Cargo.toml", manifest_text.as_str()),
        ("src/main.rs", MAIN_TEMPLATE),
        ("src/behaviours/mod.rs", BEHAVIOURS_TEMPLATE),
        (".gitignore", IGNORE_TEMPLATE),
    ];
    for (relative, text) in files {
        write_file(&root.join(relative), text.as_bytes())?;
    }
    let scene_path = root.join(ASSET_ROOT).join(STARTUP_SCENE);
    write_file(&scene_path, write_scene_text(&starter_scene()).as_bytes())?;
    ensure_asset_meta(&scene_path)?;
    let project = Project::new(
        root.to_path_buf(),
        ProjectSettings {
            name,
            asset_root: ASSET_ROOT.to_owned(),
            game_executable: format!("target/release/{package}{}", std::env::consts::EXE_SUFFIX),
            startup_scene: STARTUP_SCENE.to_owned(),
        },
    );
    project.save()?;
    Ok(project)
}
