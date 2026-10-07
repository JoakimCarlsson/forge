//! The authored data of the forge engine: JSON, persistent identifiers, typed
//! property values, entity and prefab records, the component schemas documents
//! are checked against, the scene and prefab file formats and the asset
//! metadata sidecars.
//!
//! Nothing here knows a window, a device or an engine object. Versions detect
//! mismatch and never imply compatibility: a file of another version is
//! rejected with a [`DocumentError`] naming the version found and the version
//! required. Components whose schema is unavailable keep their properties, which
//! are self-describing in the file, so they survive a save.
//!
//! Modules:
//!
//! - `guid`: the 128-bit [`Guid`]
//! - `property`, `property_json`: [`PropertyValue`] and its JSON form
//! - `records`, `nodes`: entity, component and prefab instance records, the
//!   [`EntityDocument`] holding them and the walks over its nodes
//! - `schema`, `components`, `validate`, `builtin_schema`: component types,
//!   their dependency rules and the validation of a document against them
//! - `format`, `settings`: the `.scene` and `.prefab` files
//! - `files`, `index`, `kind`: reading and atomically replacing files, the
//!   `.meta` sidecars, the [`AssetIndex`] and the [`AssetKind`] of a file
//! - `game_schema`: the export of a game's component types
//! - `json`, `error`: the JSON helpers and [`DocumentError`]

pub mod builtin_schema;
pub mod components;
mod error;
mod files;
mod format;
mod game_schema;
mod guid;
mod index;
pub mod json;
mod kind;
mod nodes;
mod property;
mod property_json;
mod records;
mod schema;
mod settings;
mod validate;

pub use builtin_schema::builtin_schemas;
pub use error::DocumentError;
pub use files::{
    AssetMeta, AssetScan, DuplicateAsset, IndexedAsset, META_FORMAT_VERSION, ensure_asset_meta,
    load_asset_meta, meta_path, read_meta_text, read_text_file, relative_text, save_asset_meta,
    scan_assets, write_file, write_meta_text,
};
pub use format::{
    FormatKind, PREFAB_FORMAT_VERSION, PrefabAsset, SCENE_FORMAT_VERSION, SceneAsset, check_format,
    read_prefab_text, read_scene_text, write_prefab_text, write_scene_text,
};
pub use game_schema::{
    GAME_SCHEMA_FORMAT_VERSION, GameSchema, read_game_schema, write_game_schema,
};
pub use guid::Guid;
pub use index::AssetIndex;
pub use kind::AssetKind;
pub use nodes::{MAX_DEPTH, NodeKind, NodeRef, effective_entity, find_override};
pub use property::{
    AssetReference, Color, ComponentReference, EntityReference, PropertyEntry, PropertyType,
    PropertyValue,
};
pub use property_json::{read_transform, write_transform};
pub use records::{
    ComponentRecord, EntityDocument, EntityRecord, PrefabInstanceRecord, PropertyOverride,
};
pub use schema::{
    ComponentSchema, ComponentSource, Diagnostic, EnumOption, PropertySchema, SchemaSet, Severity,
    has_errors,
};
pub use settings::{AmbientSettings, PhysicsSettings, SceneSettings};
pub use validate::validate_document;
