//! An open `.scene` or `.prefab` file: its content, identity and dirty state.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fr_document::{
    Diagnostic, Guid, PrefabAsset, SceneAsset, SchemaSet, ensure_asset_meta, has_errors,
    load_asset_meta, read_prefab_text, read_scene_text, read_text_file, relative_text,
    validate_document, write_file, write_prefab_text, write_scene_text,
};

use crate::apply::{ApplyContext, apply_command};
use crate::command::{Command, CommandOutcome};
use crate::data::{DocumentData, Snapshot};
use crate::error::AuthoringError;
use crate::kind::DocumentKind;

/// An open scene or prefab. Its content changes only through commands run by
/// an [`OpenDocument`](crate::OpenDocument), which records them in history;
/// saving writes it atomically and clears the dirty state.
#[derive(Clone, Debug)]
pub struct Document {
    /// Whether the file is a scene or a prefab.
    kind: DocumentKind,
    /// The absolute path of the file.
    path: PathBuf,
    /// The path relative to the asset root, with forward slashes.
    relative_path: String,
    /// The asset root the relative path is taken from.
    asset_root: PathBuf,
    /// The asset identity from the sidecar; unset until the file is saved.
    asset_id: Guid,
    /// The editable content.
    data: DocumentData,
    /// The component schemas the document is edited and validated against.
    schemas: Arc<SchemaSet>,
    /// Counts every change, previews and undo included.
    revision: u64,
    /// The identity of the current content state.
    state: u64,
    /// The state that was last written to disk; none when never saved.
    saved_state: Option<u64>,
    /// The next state identity to hand out.
    next_state: u64,
}

impl Document {
    /// Opens a `.scene` or `.prefab` file under an asset root. The sidecar is
    /// read but not created; the asset identity stays unset when there is none.
    ///
    /// # Errors
    ///
    /// When the file is neither kind, lies outside the asset root, or cannot be
    /// read or understood; version mismatches name the versions found and
    /// required.
    pub fn open(
        path: &Path,
        asset_root: &Path,
        schemas: Arc<SchemaSet>,
    ) -> Result<Self, AuthoringError> {
        let (kind, path, relative_path, asset_root) = locate(path, asset_root)?;
        let text = read_text_file(&path)?;
        let located = |error: fr_document::DocumentError| error.in_file(&path);
        let data = match kind {
            DocumentKind::Scene => {
                let asset = read_scene_text(&text).map_err(located)?;
                DocumentData {
                    content: asset.content,
                    settings: asset.settings,
                }
            }
            DocumentKind::Prefab => DocumentData {
                content: read_prefab_text(&text).map_err(located)?.content,
                ..DocumentData::default()
            },
        };
        let asset_id = load_asset_meta(&path)?.map_or(Guid::NONE, |meta| meta.id);
        let mut document = Self::assemble(
            kind,
            path,
            relative_path,
            asset_root,
            asset_id,
            data,
            schemas,
        );
        document.saved_state = Some(document.state);
        Ok(document)
    }

    /// A new, empty, unsaved scene that will be written to `path`.
    ///
    /// # Errors
    ///
    /// When the path is not a `.scene` file under the asset root or the file
    /// exists.
    pub fn new_scene(
        path: &Path,
        asset_root: &Path,
        schemas: Arc<SchemaSet>,
    ) -> Result<Self, AuthoringError> {
        Self::create(DocumentKind::Scene, path, asset_root, schemas)
    }

    /// A new, empty, unsaved prefab that will be written to `path`.
    ///
    /// # Errors
    ///
    /// When the path is not a `.prefab` file under the asset root or the file
    /// exists.
    pub fn new_prefab(
        path: &Path,
        asset_root: &Path,
        schemas: Arc<SchemaSet>,
    ) -> Result<Self, AuthoringError> {
        Self::create(DocumentKind::Prefab, path, asset_root, schemas)
    }

    /// A new unsaved document of a kind, checking that the path has that kind.
    fn create(
        kind: DocumentKind,
        path: &Path,
        asset_root: &Path,
        schemas: Arc<SchemaSet>,
    ) -> Result<Self, AuthoringError> {
        let (found, path, relative_path, asset_root) = locate(path, asset_root)?;
        if found != kind {
            return Err(AuthoringError::UnsupportedFile { path });
        }
        if path.exists() {
            return Err(AuthoringError::AlreadyExists { path });
        }
        Ok(Self::assemble(
            kind,
            path,
            relative_path,
            asset_root,
            Guid::NONE,
            DocumentData::default(),
            schemas,
        ))
    }

    /// Builds a document in state one that has never been saved.
    fn assemble(
        kind: DocumentKind,
        path: PathBuf,
        relative_path: String,
        asset_root: PathBuf,
        asset_id: Guid,
        data: DocumentData,
        schemas: Arc<SchemaSet>,
    ) -> Self {
        Self {
            kind,
            path,
            relative_path,
            asset_root,
            asset_id,
            data,
            schemas,
            revision: 0,
            state: 1,
            saved_state: None,
            next_state: 2,
        }
    }

    /// Whether the document is a scene or a prefab.
    pub fn kind(&self) -> DocumentKind {
        self.kind
    }

    /// The absolute path of the file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The path relative to the asset root, with forward slashes.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// The asset root the document lives under.
    pub fn asset_root(&self) -> &Path {
        &self.asset_root
    }

    /// The asset identity from the `.meta` sidecar; unset until the document
    /// has been saved when the file had no sidecar.
    pub fn asset_id(&self) -> Guid {
        self.asset_id
    }

    /// The name shown for the document: the file name.
    pub fn title(&self) -> String {
        self.path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    }

    /// The editable content: entity records, instances and scene settings.
    pub fn data(&self) -> &DocumentData {
        &self.data
    }

    /// The entities and prefab instances.
    pub fn content(&self) -> &fr_document::EntityDocument {
        &self.data.content
    }

    /// The scene settings; the defaults for a prefab.
    pub fn settings(&self) -> &fr_document::SceneSettings {
        &self.data.settings
    }

    /// The component schemas the document is edited against.
    pub fn schemas(&self) -> &Arc<SchemaSet> {
        &self.schemas
    }

    /// Replaces the schema set, after a game's types were loaded or changed.
    pub fn set_schemas(&mut self, schemas: Arc<SchemaSet>) {
        self.schemas = schemas;
    }

    /// The number of changes so far. It grows on every change, previews and
    /// undo and redo included, so a view can tell that it is out of date.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether the content differs from what was last saved or opened.
    pub fn is_dirty(&self) -> bool {
        self.saved_state != Some(self.state)
    }

    /// The identity of the current content state.
    pub(crate) fn state(&self) -> u64 {
        self.state
    }

    /// What validation finds in the content, errors and warnings.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        validate_document(&self.data.content, &self.schemas)
    }

    /// The text of the file as a save would write it.
    pub fn to_text(&self) -> String {
        match self.kind {
            DocumentKind::Scene => write_scene_text(&SceneAsset {
                id: self.asset_id,
                content: self.data.content.clone(),
                settings: self.data.settings,
            }),
            DocumentKind::Prefab => write_prefab_text(&PrefabAsset {
                id: self.asset_id,
                content: self.data.content.clone(),
            }),
        }
    }

    /// Validates the content and writes the file atomically with its `.meta`
    /// sidecar, creating the sidecar when the file has none. Components of
    /// types whose schema is unavailable are written as they were read.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::Invalid`] listing the validation errors, or the
    /// failure of the first file that cannot be written; the document stays
    /// dirty in both cases.
    pub fn save(&mut self) -> Result<(), AuthoringError> {
        let diagnostics = self.diagnostics();
        if has_errors(&diagnostics) {
            return Err(AuthoringError::invalid(&diagnostics));
        }
        let meta = ensure_asset_meta(&self.path)?;
        self.asset_id = meta.id;
        write_file(&self.path, self.to_text().as_bytes())?;
        self.saved_state = Some(self.state);
        Ok(())
    }

    /// Runs a command against the content. An unchanged result leaves the
    /// document as it was; otherwise the revision grows and the content takes a
    /// new state.
    ///
    /// # Errors
    ///
    /// The command's rejection; the content is untouched.
    pub(crate) fn execute(&mut self, command: &Command) -> Result<CommandOutcome, AuthoringError> {
        let context = ApplyContext {
            schemas: &self.schemas,
            document_asset: self.asset_id,
        };
        let mut working = self.data.clone();
        let created = apply_command(command, &mut working, &context)?;
        let changed = working != self.data;
        if changed {
            self.data = working;
            self.revision += 1;
            self.state = self.fresh_state();
        }
        Ok(CommandOutcome { created, changed })
    }

    /// The current content and state, to restore later.
    pub(crate) fn snapshot(&self) -> Snapshot {
        Snapshot {
            data: self.data.clone(),
            state: self.state,
        }
    }

    /// Puts content and state back from a snapshot, as one change.
    pub(crate) fn restore(&mut self, snapshot: &Snapshot) {
        self.data = snapshot.data.clone();
        self.state = snapshot.state;
        self.revision += 1;
    }

    /// A state identity no earlier state had.
    pub(crate) fn fresh_state(&mut self) -> u64 {
        let state = self.next_state;
        self.next_state += 1;
        state
    }
}

/// Resolves a file path against an asset root into its kind, absolute path,
/// root-relative path and absolute asset root.
///
/// # Errors
///
/// When the file is neither a scene nor a prefab, or lies outside the root.
fn locate(
    path: &Path,
    asset_root: &Path,
) -> Result<(DocumentKind, PathBuf, String, PathBuf), AuthoringError> {
    let kind = DocumentKind::of_path(path).ok_or_else(|| AuthoringError::UnsupportedFile {
        path: path.to_path_buf(),
    })?;
    let absolute = absolute_path(path);
    let root = absolute_path(asset_root);
    if !absolute.starts_with(&root) {
        return Err(AuthoringError::OutsideAssetRoot {
            path: absolute,
            asset_root: root,
        });
    }
    let relative = relative_text(&root, &absolute);
    Ok((kind, absolute, relative, root))
}

/// The absolute form of a path, canonical when the path exists.
pub(crate) fn absolute_path(path: &Path) -> PathBuf {
    path.canonicalize()
        .or_else(|_| {
            let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
            match (parent.map(Path::canonicalize), path.file_name()) {
                (Some(Ok(parent)), Some(name)) => Ok(parent.join(name)),
                _ => std::path::absolute(path),
            }
        })
        .unwrap_or_else(|_| path.to_path_buf())
}
