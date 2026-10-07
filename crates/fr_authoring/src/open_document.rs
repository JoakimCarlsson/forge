//! A document with its own history and selection: the unit an editor works on.

use crate::command::{CoalesceKey, Command, CommandOutcome};
use crate::document::Document;
use crate::error::AuthoringError;
use crate::history::History;
use crate::selection::Selection;

/// A [`Document`] paired with its [`History`] and [`Selection`]. Every edit
/// goes through here, so the selection is pruned after anything that can remove
/// nodes: a command, an undo or redo, a gesture's commit or cancel.
#[derive(Clone, Debug)]
pub struct OpenDocument {
    /// The document.
    document: Document,
    /// Its undo and redo stacks.
    history: History,
    /// Its selected nodes.
    selection: Selection,
}

impl OpenDocument {
    /// Wraps a document with an empty history and selection.
    pub fn new(document: Document) -> Self {
        Self {
            document,
            history: History::new(),
            selection: Selection::new(),
        }
    }

    /// The document.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The document, to save it or replace its schemas. Content changes only go
    /// through commands.
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    /// The undo and redo stacks.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// The selected nodes.
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// The selected nodes, to change the selection.
    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    /// Whether the document has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.document.is_dirty()
    }

    /// The name shown for the document.
    pub fn title(&self) -> String {
        self.document.title()
    }

    /// Removes selected nodes that no longer exist.
    fn prune_selection(&mut self) {
        self.selection.prune(self.document.content());
    }

    /// Runs a command as one undo entry.
    ///
    /// # Errors
    ///
    /// As [`History::execute`].
    pub fn execute(&mut self, command: &Command) -> Result<CommandOutcome, AuthoringError> {
        let outcome = self.history.execute(&mut self.document, command);
        self.prune_selection();
        outcome
    }

    /// Runs a command, merging it into the previous entry when they share a
    /// key.
    ///
    /// # Errors
    ///
    /// As [`History::apply_coalescing`].
    pub fn apply_coalescing(
        &mut self,
        command: &Command,
        key: CoalesceKey,
    ) -> Result<CommandOutcome, AuthoringError> {
        let outcome = self
            .history
            .apply_coalescing(&mut self.document, command, key);
        self.prune_selection();
        outcome
    }

    /// Runs a command, coalescing it when it is a property or transform edit
    /// and recording it plainly otherwise.
    ///
    /// # Errors
    ///
    /// As [`History::execute`].
    pub fn execute_coalescing(
        &mut self,
        command: &Command,
    ) -> Result<CommandOutcome, AuthoringError> {
        match command.coalesce_key() {
            Some(key) => self.apply_coalescing(command, key),
            None => self.execute(command),
        }
    }

    /// Undoes the last entry, returning its label.
    ///
    /// # Errors
    ///
    /// As [`History::undo`].
    pub fn undo(&mut self) -> Result<Option<String>, AuthoringError> {
        let result = self.history.undo(&mut self.document);
        self.prune_selection();
        result
    }

    /// Redoes the last undone entry, returning its label.
    ///
    /// # Errors
    ///
    /// As [`History::redo`].
    pub fn redo(&mut self) -> Result<Option<String>, AuthoringError> {
        let result = self.history.redo(&mut self.document);
        self.prune_selection();
        result
    }

    /// Starts a gesture.
    ///
    /// # Errors
    ///
    /// As [`History::begin_gesture`].
    pub fn begin_gesture(&mut self, label: impl Into<String>) -> Result<(), AuthoringError> {
        self.history.begin_gesture(&self.document, label)
    }

    /// Applies a command live during a gesture.
    ///
    /// # Errors
    ///
    /// As [`History::preview`].
    pub fn preview(&mut self, command: &Command) -> Result<CommandOutcome, AuthoringError> {
        let outcome = self.history.preview(&mut self.document, command);
        self.prune_selection();
        outcome
    }

    /// Records the gesture as one entry; returns whether anything changed.
    ///
    /// # Errors
    ///
    /// As [`History::commit_gesture`].
    pub fn commit_gesture(&mut self) -> Result<bool, AuthoringError> {
        self.history.commit_gesture(&mut self.document)
    }

    /// Restores the document to where the gesture began.
    ///
    /// # Errors
    ///
    /// As [`History::cancel_gesture`].
    pub fn cancel_gesture(&mut self) -> Result<(), AuthoringError> {
        let result = self.history.cancel_gesture(&mut self.document);
        self.prune_selection();
        result
    }

    /// Saves the document.
    ///
    /// # Errors
    ///
    /// As [`Document::save`].
    pub fn save(&mut self) -> Result<(), AuthoringError> {
        self.document.save()
    }
}
