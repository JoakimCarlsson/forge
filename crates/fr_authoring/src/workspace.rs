//! The set of open documents of an editing session.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fr_document::SchemaSet;

use crate::document::{Document, absolute_path};
use crate::error::AuthoringError;
use crate::open_document::OpenDocument;

/// A stable handle to an open document; it stays valid until the document is
/// closed and is never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(u64);

impl DocumentId {
    /// The number behind the handle.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// The outcome of saving one document.
#[derive(Debug)]
pub struct SaveResult {
    /// The document that was saved.
    pub id: DocumentId,
    /// Its file.
    pub path: PathBuf,
    /// Whether it was written.
    pub result: Result<(), AuthoringError>,
}

/// The documents of one project that are open, each with its own history and
/// selection, and which of them is current.
#[derive(Debug)]
pub struct Workspace {
    /// The asset root documents are opened under.
    asset_root: PathBuf,
    /// The schemas new documents are edited against.
    schemas: Arc<SchemaSet>,
    /// The open documents in the order they were opened.
    documents: Vec<(DocumentId, OpenDocument)>,
    /// The current document.
    current: Option<DocumentId>,
    /// The next handle to hand out.
    next_id: u64,
}

impl Workspace {
    /// An empty workspace over an asset root and a schema set.
    pub fn new(asset_root: PathBuf, schemas: Arc<SchemaSet>) -> Self {
        Self {
            asset_root,
            schemas,
            documents: Vec::new(),
            current: None,
            next_id: 1,
        }
    }

    /// The asset root documents are opened under.
    pub fn asset_root(&self) -> &Path {
        &self.asset_root
    }

    /// The schema set shared by the documents.
    pub fn schemas(&self) -> &Arc<SchemaSet> {
        &self.schemas
    }

    /// Replaces the schema set of the workspace and of every open document.
    pub fn set_schemas(&mut self, schemas: Arc<SchemaSet>) {
        for (_, open) in &mut self.documents {
            open.document_mut().set_schemas(Arc::clone(&schemas));
        }
        self.schemas = schemas;
    }

    /// Opens a file and makes it current; a file that is open already is not
    /// read again and its existing handle is returned, made current.
    ///
    /// # Errors
    ///
    /// When the file cannot be opened as a document.
    pub fn open(&mut self, path: &Path) -> Result<DocumentId, AuthoringError> {
        if let Some(existing) = self.find_by_path(path) {
            self.current = Some(existing);
            return Ok(existing);
        }
        let document = Document::open(path, &self.asset_root, Arc::clone(&self.schemas))?;
        Ok(self.add(OpenDocument::new(document)))
    }

    /// Creates a new empty scene at a path, unsaved, and makes it current.
    ///
    /// # Errors
    ///
    /// As [`Document::new_scene`], or when the path is open already.
    pub fn new_scene(&mut self, path: &Path) -> Result<DocumentId, AuthoringError> {
        self.reject_open(path)?;
        let document = Document::new_scene(path, &self.asset_root, Arc::clone(&self.schemas))?;
        Ok(self.add(OpenDocument::new(document)))
    }

    /// Creates a new empty prefab at a path, unsaved, and makes it current.
    ///
    /// # Errors
    ///
    /// As [`Document::new_prefab`], or when the path is open already.
    pub fn new_prefab(&mut self, path: &Path) -> Result<DocumentId, AuthoringError> {
        self.reject_open(path)?;
        let document = Document::new_prefab(path, &self.asset_root, Arc::clone(&self.schemas))?;
        Ok(self.add(OpenDocument::new(document)))
    }

    /// Checks that no open document has the path.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::AlreadyExists`] when one has.
    fn reject_open(&self, path: &Path) -> Result<(), AuthoringError> {
        if self.find_by_path(path).is_some() {
            return Err(AuthoringError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }
        Ok(())
    }

    /// Adds a document and makes it current.
    pub fn add(&mut self, document: OpenDocument) -> DocumentId {
        let id = DocumentId(self.next_id);
        self.next_id += 1;
        self.documents.push((id, document));
        self.current = Some(id);
        id
    }

    /// Closes a document without saving, returning it. When it was current, the
    /// document that was open before it, or after it when none was, becomes
    /// current.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::UnknownDocument`] when it is not open.
    pub fn close(&mut self, id: DocumentId) -> Result<OpenDocument, AuthoringError> {
        let index = self.index_of(id)?;
        let (_, closed) = self.documents.remove(index);
        if self.current == Some(id) {
            let neighbour = index
                .saturating_sub(1)
                .min(self.documents.len().saturating_sub(1));
            self.current = self.documents.get(neighbour).map(|(other, _)| *other);
        }
        Ok(closed)
    }

    /// Makes a document current.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::UnknownDocument`] when it is not open.
    pub fn set_current(&mut self, id: DocumentId) -> Result<(), AuthoringError> {
        self.index_of(id)?;
        self.current = Some(id);
        Ok(())
    }

    /// The position of an open document.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::UnknownDocument`] when it is not open.
    fn index_of(&self, id: DocumentId) -> Result<usize, AuthoringError> {
        self.documents
            .iter()
            .position(|(candidate, _)| *candidate == id)
            .ok_or(AuthoringError::UnknownDocument { id })
    }

    /// The handle of the current document.
    pub fn current_id(&self) -> Option<DocumentId> {
        self.current
    }

    /// The current document.
    pub fn current(&self) -> Option<&OpenDocument> {
        self.current.and_then(|id| self.get(id))
    }

    /// The current document, to edit.
    pub fn current_mut(&mut self) -> Option<&mut OpenDocument> {
        let id = self.current?;
        self.get_mut(id)
    }

    /// An open document.
    pub fn get(&self, id: DocumentId) -> Option<&OpenDocument> {
        self.documents
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .map(|(_, open)| open)
    }

    /// An open document, to edit.
    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut OpenDocument> {
        self.documents
            .iter_mut()
            .find(|(candidate, _)| *candidate == id)
            .map(|(_, open)| open)
    }

    /// The handle of the open document for a file.
    pub fn find_by_path(&self, path: &Path) -> Option<DocumentId> {
        let wanted = absolute_path(path);
        self.documents
            .iter()
            .find(|(_, open)| open.document().path() == wanted)
            .map(|(id, _)| *id)
    }

    /// The handles of the open documents in the order they were opened.
    pub fn ids(&self) -> Vec<DocumentId> {
        self.documents.iter().map(|(id, _)| *id).collect()
    }

    /// The open documents with their handles, in the order they were opened.
    pub fn documents(&self) -> impl Iterator<Item = (DocumentId, &OpenDocument)> {
        self.documents.iter().map(|(id, open)| (*id, open))
    }

    /// The number of open documents.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Whether no document is open.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// The handles of the documents with unsaved changes.
    pub fn dirty_documents(&self) -> Vec<DocumentId> {
        self.documents
            .iter()
            .filter(|(_, open)| open.is_dirty())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Saves one document.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::UnknownDocument`] when it is not open, or the save's
    /// failure.
    pub fn save(&mut self, id: DocumentId) -> Result<(), AuthoringError> {
        self.get_mut(id)
            .ok_or(AuthoringError::UnknownDocument { id })?
            .save()
    }

    /// Saves every dirty document, one result each; a failure does not stop the
    /// others.
    pub fn save_all(&mut self) -> Vec<SaveResult> {
        self.documents
            .iter_mut()
            .filter(|(_, open)| open.is_dirty())
            .map(|(id, open)| SaveResult {
                id: *id,
                path: open.document().path().to_path_buf(),
                result: open.save(),
            })
            .collect()
    }
}
