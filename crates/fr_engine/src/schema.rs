//! The export of a game's component schema, which a tool reads to know the
//! game's types without loading game code.

use std::path::Path;

use fr_document::{GameSchema, write_file, write_game_schema};

use crate::error::{EngineError, EngineResult};
use crate::registry::ComponentRegistry;

/// The command line option that asks a game to write its schema and exit.
pub const EXPORT_SCHEMA_OPTION: &str = "--forge-export-schema";

/// The schema text of a registry's game types.
pub fn export_game_schema(registry: &ComponentRegistry) -> String {
    write_game_schema(&GameSchema::from_set(registry.schemas()))
}

/// Writes the schema of a registry's game types to a file.
///
/// # Errors
///
/// When the file cannot be written.
pub fn save_game_schema(registry: &ComponentRegistry, path: &Path) -> EngineResult {
    write_file(path, export_game_schema(registry).as_bytes())?;
    Ok(())
}

/// Writes the game schema and returns `true` when the command line carries
/// `--forge-export-schema <file>`, in which case `main` returns without opening
/// a window or constructing a behaviour. Returns `false` when it does not.
///
/// # Errors
///
/// When the option has no file after it or the file cannot be written.
pub fn export_schema_if_requested(
    registry: &ComponentRegistry,
    arguments: &[String],
) -> EngineResult<bool> {
    let Some(position) = arguments
        .iter()
        .position(|argument| argument == EXPORT_SCHEMA_OPTION)
    else {
        return Ok(false);
    };
    let path = arguments.get(position + 1).ok_or_else(|| {
        EngineError::new(format!("{EXPORT_SCHEMA_OPTION} needs the file to write"))
    })?;
    save_game_schema(registry, Path::new(path))?;
    Ok(true)
}
