//! UTF-8 asset documents read and written independently of a graphics device.

use std::path::Path;

use crate::AssetError;

/// Reads a UTF-8 asset document.
///
/// # Errors
///
/// Returns an error when the file cannot be read or is not UTF-8.
pub fn load_text(path: impl AsRef<Path>) -> Result<String, AssetError> {
    let path = path.as_ref();
    std::fs::read_to_string(path).map_err(|error| AssetError::import(path, error))
}

/// Writes a UTF-8 asset document.
///
/// # Errors
///
/// Returns an error when the destination cannot be written.
pub fn save_text(path: impl AsRef<Path>, source: &str) -> Result<(), AssetError> {
    let path = path.as_ref();
    std::fs::write(path, source).map_err(|error| AssetError::import(path, error))
}
