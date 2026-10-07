//! Reading and atomically replacing authored files, and the metadata sidecar
//! that gives every asset its identity.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use crate::error::DocumentError;
use crate::format::check_format;
use crate::guid::Guid;
use crate::json::{self, as_object, guid_value, object, required};

/// The format name of a metadata sidecar.
const META_FORMAT: &str = "forge.meta";

/// The version every metadata sidecar is written at and read at.
pub const META_FORMAT_VERSION: u32 = 1;

/// The extension appended to an asset path to name its metadata sidecar.
const META_EXTENSION: &str = "meta";

/// Directories a scan of a project never enters.
const SKIPPED_DIRECTORIES: [&str; 2] = ["target", "build"];

/// The identity of an asset, as its sidecar stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetMeta {
    /// The asset identity references store.
    pub id: Guid,
}

/// The path of the sidecar of an asset: the asset path with `.meta` appended.
pub fn meta_path(asset: &Path) -> PathBuf {
    let mut name = asset.as_os_str().to_owned();
    name.push(".");
    name.push(META_EXTENSION);
    PathBuf::from(name)
}

/// Serialises a sidecar.
pub fn write_meta_text(meta: &AssetMeta) -> String {
    let mut map = object();
    map.insert("format".to_owned(), Value::String(META_FORMAT.to_owned()));
    map.insert(
        "format_version".to_owned(),
        Value::from(i64::from(META_FORMAT_VERSION)),
    );
    map.insert("id".to_owned(), guid_value(meta.id));
    json::write(&Value::Object(map))
}

/// Parses a sidecar.
///
/// # Errors
///
/// When the text is not a sidecar of the current version or carries no valid
/// identity.
pub fn read_meta_text(source: &str) -> Result<AssetMeta, DocumentError> {
    let value = json::parse(source)?;
    let map = as_object(&value, "meta")?;
    check_format(map, META_FORMAT, META_FORMAT_VERSION)?;
    let id = json::as_guid(required(map, "id", "meta")?, "meta.id")?;
    if !id.valid() {
        return Err(DocumentError::malformed("meta.id: the identity is unset"));
    }
    Ok(AssetMeta { id })
}

/// Reads a file as UTF-8 text.
///
/// # Errors
///
/// When the file cannot be read, or is not UTF-8 text, naming the byte offset.
pub fn read_text_file(path: &Path) -> Result<String, DocumentError> {
    let bytes = fs::read(path).map_err(|source| DocumentError::io(path, source))?;
    String::from_utf8(bytes).map_err(|error| {
        DocumentError::NotUtf8 {
            offset: error.utf8_error().valid_up_to(),
        }
        .in_file(path)
    })
}

/// A file name no other writer in this process uses beside `path`.
fn temporary_path(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(".{name}.{}.{unique}.tmp", std::process::id()))
}

/// Writes a file by writing a uniquely named temporary file in the destination
/// directory, flushing it and replacing the target with it, so the original is
/// never truncated. Missing parent directories are created.
///
/// # Errors
///
/// When a directory or the file cannot be written; the temporary file is
/// removed.
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), DocumentError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| DocumentError::io(parent, source))?;
    }
    let temporary = temporary_path(path);
    let written = fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    let replaced = written.and_then(|()| fs::rename(&temporary, path));
    if let Err(source) = replaced {
        let _ = fs::remove_file(&temporary);
        return Err(DocumentError::io(path, source));
    }
    Ok(())
}

/// Reads the sidecar of an asset; none when it has none.
///
/// # Errors
///
/// When the sidecar exists and cannot be read or is not of the current version.
pub fn load_asset_meta(asset: &Path) -> Result<Option<AssetMeta>, DocumentError> {
    let path = meta_path(asset);
    if !path.exists() {
        return Ok(None);
    }
    let text = read_text_file(&path)?;
    read_meta_text(&text)
        .map(Some)
        .map_err(|error| error.in_file(&path))
}

/// Writes the sidecar of an asset.
///
/// # Errors
///
/// When the sidecar cannot be written.
pub fn save_asset_meta(asset: &Path, meta: &AssetMeta) -> Result<(), DocumentError> {
    write_file(&meta_path(asset), write_meta_text(meta).as_bytes())
}

/// Reads the sidecar of an asset, writing one with a fresh identity when the
/// asset has none.
///
/// # Errors
///
/// When the sidecar cannot be read or written.
pub fn ensure_asset_meta(asset: &Path) -> Result<AssetMeta, DocumentError> {
    if let Some(meta) = load_asset_meta(asset)? {
        return Ok(meta);
    }
    let meta = AssetMeta {
        id: Guid::generate(),
    };
    save_asset_meta(asset, &meta)?;
    Ok(meta)
}

/// An asset found under a root: its path and identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedAsset {
    /// The path relative to the scanned root, with forward slashes.
    pub path: String,
    /// The identity from the sidecar.
    pub id: Guid,
}

/// Two assets that claim one identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateAsset {
    /// The shared identity.
    pub id: Guid,
    /// The path that was found first.
    pub first: String,
    /// The path that was found later and is not indexed.
    pub second: String,
}

/// The assets under a root, in path order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetScan {
    /// The assets with a valid sidecar and a file beside it.
    pub assets: Vec<IndexedAsset>,
    /// The assets whose identity another asset already holds.
    pub duplicates: Vec<DuplicateAsset>,
    /// Sidecars that could not be read, with the reason.
    pub problems: Vec<String>,
}

/// A path relative to a root as text with forward slashes.
pub fn relative_text(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The entries of a directory in name order.
fn sorted_entries(directory: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .map(|reader| {
            reader
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    entries
}

/// Whether a directory is entered by a scan.
fn scans_directory(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        !name.starts_with('.') && !SKIPPED_DIRECTORIES.contains(&name.as_ref())
    })
}

/// Collects the assets of a directory and everything below it.
fn scan_directory(root: &Path, directory: &Path, scan: &mut AssetScan) {
    for path in sorted_entries(directory) {
        if path.is_dir() {
            if scans_directory(&path) {
                scan_directory(root, &path, scan);
            }
            continue;
        }
        if path
            .extension()
            .is_none_or(|extension| extension != META_EXTENSION)
        {
            continue;
        }
        let asset = path.with_extension("");
        if !asset.is_file() {
            continue;
        }
        let relative = relative_text(root, &asset);
        match load_asset_meta(&asset) {
            Ok(Some(meta)) => match scan.assets.iter().find(|found| found.id == meta.id) {
                Some(first) => scan.duplicates.push(DuplicateAsset {
                    id: meta.id,
                    first: first.path.clone(),
                    second: relative,
                }),
                None => scan.assets.push(IndexedAsset {
                    path: relative,
                    id: meta.id,
                }),
            },
            Ok(None) => {}
            Err(error) => scan.problems.push(error.to_string()),
        }
    }
}

/// Finds every asset under a root by its sidecar.
pub fn scan_assets(root: &Path) -> AssetScan {
    let mut scan = AssetScan::default();
    scan_directory(root, root, &mut scan);
    scan.assets.sort_by(|a, b| a.path.cmp(&b.path));
    scan
}
