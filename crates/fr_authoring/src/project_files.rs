//! File operations behind the Project panel: creating folders, scenes and
//! prefabs with their `.meta` sidecars, and building the commands that place an
//! asset in a document.

use std::fs;
use std::path::{Path, PathBuf};

use fr_document::{
    AssetKind, AssetReference, Guid, PropertyValue, builtin_schema, ensure_asset_meta,
    relative_text, write_file, write_prefab_text, write_scene_text,
};
use fr_transform::Transform;

use crate::command::{AddPrefabInstance, ComponentInit, CreateEntity};
use crate::document::absolute_path;
use crate::error::AuthoringError;
use crate::kind::DocumentKind;
use crate::starter::{empty_scene, starter_prefab, starter_scene};

/// The content a new scene file starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneTemplate {
    /// A camera and a directional sun.
    Starter,
    /// No entities.
    Empty,
}

/// A file that was created, with its identity from the sidecar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedAsset {
    /// The absolute path of the file.
    pub path: PathBuf,
    /// The path relative to the asset root, with forward slashes.
    pub relative_path: String,
    /// The asset identity.
    pub id: Guid,
}

/// The longest file or folder name accepted.
const MAX_NAME_LENGTH: usize = 255;

/// Checks that a name can be one path segment.
///
/// # Errors
///
/// [`AuthoringError::InvalidName`] for an empty name, a path separator, a dot
/// name, a leading dot or a control character.
fn validate_name(name: &str) -> Result<&str, AuthoringError> {
    let trimmed = name.trim();
    let reason = if trimmed.is_empty() {
        Some("the name is empty")
    } else if trimmed.contains(['/', '\\']) {
        Some("the name contains a path separator")
    } else if trimmed.starts_with('.') {
        Some("the name starts with a dot")
    } else if trimmed.chars().any(char::is_control) {
        Some("the name contains a control character")
    } else if trimmed.len() > MAX_NAME_LENGTH {
        Some("the name is too long")
    } else {
        None
    };
    match reason {
        Some(reason) => Err(AuthoringError::InvalidName {
            name: name.to_owned(),
            reason: reason.to_owned(),
        }),
        None => Ok(trimmed),
    }
}

/// The absolute form of a directory that exists under the asset root.
///
/// # Errors
///
/// [`AuthoringError::OutsideAssetRoot`] when it lies outside the root, or
/// [`AuthoringError::InvalidName`] when it is not a directory.
fn directory_under(asset_root: &Path, directory: &Path) -> Result<PathBuf, AuthoringError> {
    let root = absolute_path(asset_root);
    let absolute = absolute_path(directory);
    if !absolute.starts_with(&root) {
        return Err(AuthoringError::OutsideAssetRoot {
            path: absolute,
            asset_root: root,
        });
    }
    if !absolute.is_dir() {
        return Err(AuthoringError::InvalidName {
            name: absolute.display().to_string(),
            reason: "it is not a folder".to_owned(),
        });
    }
    Ok(absolute)
}

/// A new path in a directory for a name and extension, refusing an existing
/// one.
///
/// # Errors
///
/// When the name or directory is not acceptable or the path exists.
fn new_path(
    asset_root: &Path,
    directory: &Path,
    name: &str,
    extension: Option<&str>,
) -> Result<PathBuf, AuthoringError> {
    let name = validate_name(name)?;
    let directory = directory_under(asset_root, directory)?;
    let file_name = match extension {
        Some(extension) if !name.ends_with(&format!(".{extension}")) => {
            format!("{name}.{extension}")
        }
        _ => name.to_owned(),
    };
    let path = directory.join(file_name);
    if path.exists() {
        return Err(AuthoringError::AlreadyExists { path });
    }
    Ok(path)
}

/// Writes a new asset file and its sidecar and describes it.
///
/// # Errors
///
/// When the file or sidecar cannot be written.
fn write_asset(
    asset_root: &Path,
    path: PathBuf,
    text: &str,
) -> Result<CreatedAsset, AuthoringError> {
    write_file(&path, text.as_bytes())?;
    let meta = ensure_asset_meta(&path)?;
    Ok(CreatedAsset {
        relative_path: relative_text(&absolute_path(asset_root), &path),
        path,
        id: meta.id,
    })
}

/// Creates a folder in a directory of the asset root.
///
/// # Errors
///
/// When the name is not acceptable, the directory lies outside the asset root
/// or the folder exists, or the file system refuses.
pub fn create_folder(
    asset_root: &Path,
    directory: &Path,
    name: &str,
) -> Result<PathBuf, AuthoringError> {
    let path = new_path(asset_root, directory, name, None)?;
    fs::create_dir(&path).map_err(|source| fr_document::DocumentError::io(&path, source))?;
    Ok(path)
}

/// Creates a `.scene` file and its `.meta` sidecar in a directory of the asset
/// root.
///
/// # Errors
///
/// As [`create_folder`], or when the files cannot be written.
pub fn create_scene_file(
    asset_root: &Path,
    directory: &Path,
    name: &str,
    template: SceneTemplate,
) -> Result<CreatedAsset, AuthoringError> {
    let path = new_path(
        asset_root,
        directory,
        name,
        Some(DocumentKind::Scene.extension()),
    )?;
    let scene = match template {
        SceneTemplate::Starter => starter_scene(),
        SceneTemplate::Empty => empty_scene(),
    };
    write_asset(asset_root, path, &write_scene_text(&scene))
}

/// Creates a `.prefab` file with one root entity named after the file, and its
/// `.meta` sidecar, in a directory of the asset root.
///
/// # Errors
///
/// As [`create_folder`], or when the files cannot be written.
pub fn create_prefab_file(
    asset_root: &Path,
    directory: &Path,
    name: &str,
) -> Result<CreatedAsset, AuthoringError> {
    let path = new_path(
        asset_root,
        directory,
        name,
        Some(DocumentKind::Prefab.extension()),
    )?;
    let stem = path
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    write_asset(asset_root, path, &write_prefab_text(&starter_prefab(&stem)))
}

/// The reference to an asset file under the asset root, creating its sidecar
/// when it has none.
///
/// # Errors
///
/// When the file lies outside the asset root or its sidecar cannot be read or
/// written.
pub fn asset_reference(asset_root: &Path, asset: &Path) -> Result<AssetReference, AuthoringError> {
    let root = absolute_path(asset_root);
    let absolute = absolute_path(asset);
    if !absolute.starts_with(&root) {
        return Err(AuthoringError::OutsideAssetRoot {
            path: absolute,
            asset_root: root,
        });
    }
    let meta = ensure_asset_meta(&absolute)?;
    Ok(AssetReference {
        asset: meta.id,
        last_known_path: relative_text(&root, &absolute),
    })
}

/// The file stem of a path as text.
fn stem_text(path: &Path) -> String {
    path.file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

/// The command that creates a mesh renderer entity for a glTF model: named
/// after the model file, with a `forge.mesh_renderer` whose `model` references
/// the asset.
///
/// # Errors
///
/// When the file is not a model, lies outside the asset root, or its sidecar
/// cannot be read or written.
pub fn model_entity_command(
    asset_root: &Path,
    model: &Path,
    parent: Guid,
) -> Result<CreateEntity, AuthoringError> {
    if AssetKind::of_path(&model.to_string_lossy()) != AssetKind::Model {
        return Err(AuthoringError::UnsupportedFile {
            path: model.to_path_buf(),
        });
    }
    let reference = asset_reference(asset_root, model)?;
    let renderer = ComponentInit::new(builtin_schema::MESH_RENDERER)
        .with("model", PropertyValue::Asset(reference));
    Ok(CreateEntity::new(stem_text(model))
        .under(parent)
        .with_component(renderer))
}

/// The command that places an instance of a prefab file, named after it.
///
/// # Errors
///
/// When the file is not a prefab, lies outside the asset root, or its sidecar
/// cannot be read or written.
pub fn prefab_instance_command(
    asset_root: &Path,
    prefab: &Path,
    parent: Guid,
) -> Result<AddPrefabInstance, AuthoringError> {
    if DocumentKind::of_path(prefab) != Some(DocumentKind::Prefab) {
        return Err(AuthoringError::UnsupportedFile {
            path: prefab.to_path_buf(),
        });
    }
    Ok(AddPrefabInstance {
        prefab: asset_reference(asset_root, prefab)?,
        name: stem_text(prefab),
        parent,
        index: None,
        transform: Transform::IDENTITY,
    })
}
