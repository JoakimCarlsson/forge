//! The undo and redo stacks of one document, with the gesture protocol and
//! coalescing of repeated edits.

use crate::command::{CoalesceKey, Command, CommandOutcome};
use crate::data::Snapshot;
use crate::document::Document;
use crate::error::AuthoringError;

/// The most entries the undo stack keeps; the oldest are dropped past it.
const MAX_ENTRIES: usize = 256;

/// How many revisions may pass between two edits for the second to merge into
/// the first.
const COALESCE_REVISION_WINDOW: u64 = 8;

/// One undo step: the content before and after it, with its label.
#[derive(Clone, Debug)]
struct Entry {
    /// The label shown for the step.
    label: String,
    /// The content the step started from.
    before: Snapshot,
    /// The content the step produced.
    after: Snapshot,
    /// The document revision when the step was last extended.
    revision: u64,
    /// The key later edits merge under, when the step was a coalescing edit.
    key: Option<CoalesceKey>,
}

/// A gesture in progress: its label and the content it began from.
#[derive(Clone, Debug)]
struct Gesture {
    /// The label of the entry the gesture commits as.
    label: String,
    /// The content when the gesture began.
    base: Snapshot,
}

/// The undo and redo stacks of a document. The history does not own the
/// document; every operation takes it, so the history and the document it
/// belongs to stay a pair.
#[derive(Clone, Debug, Default)]
pub struct History {
    /// Steps that can be undone, oldest first.
    undo: Vec<Entry>,
    /// Steps that can be redone, the next one last.
    redo: Vec<Entry>,
    /// The gesture in progress.
    gesture: Option<Gesture>,
}

impl History {
    /// An empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs a command as one undo entry. A command that changes nothing records
    /// nothing, and a new entry discards the redo stack.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::GestureActive`] during a gesture, or the command's
    /// rejection, which leaves the document and history as they were.
    pub fn execute(
        &mut self,
        document: &mut Document,
        command: &Command,
    ) -> Result<CommandOutcome, AuthoringError> {
        self.reject_during_gesture()?;
        self.run(document, command, None)
    }

    /// Runs a command like [`History::execute`], but merges it into the last
    /// entry when that entry has the same key, is the current state and is
    /// recent. A run of edits to one value then undoes in one step, and an edit
    /// that returns the value to where the entry began removes the entry.
    ///
    /// # Errors
    ///
    /// As [`History::execute`].
    pub fn apply_coalescing(
        &mut self,
        document: &mut Document,
        command: &Command,
        key: CoalesceKey,
    ) -> Result<CommandOutcome, AuthoringError> {
        self.reject_during_gesture()?;
        if !self.can_merge(document, &key) {
            return self.run(document, command, Some(key));
        }
        let outcome = document.execute(command)?;
        if !outcome.changed {
            return Ok(outcome);
        }
        let Some(entry) = self.undo.last_mut() else {
            return Ok(outcome);
        };
        entry.after = document.snapshot();
        entry.revision = document.revision();
        if entry.after.data == entry.before.data {
            let before = entry.before.clone();
            self.undo.pop();
            document.restore(&before);
        }
        Ok(outcome)
    }

    /// Whether the last entry can absorb an edit under a key now.
    fn can_merge(&self, document: &Document, key: &CoalesceKey) -> bool {
        self.undo.last().is_some_and(|entry| {
            entry.key.as_ref() == Some(key)
                && entry.after.state == document.state()
                && document.revision().saturating_sub(entry.revision) <= COALESCE_REVISION_WINDOW
        })
    }

    /// Runs a command and records it as a new entry when it changed the
    /// document.
    fn run(
        &mut self,
        document: &mut Document,
        command: &Command,
        key: Option<CoalesceKey>,
    ) -> Result<CommandOutcome, AuthoringError> {
        let before = document.snapshot();
        let outcome = document.execute(command)?;
        if outcome.changed {
            self.push(Entry {
                label: command.label(),
                before,
                after: document.snapshot(),
                revision: document.revision(),
                key,
            });
        }
        Ok(outcome)
    }

    /// Records an entry, dropping the redo stack and the oldest entries past
    /// the limit.
    fn push(&mut self, entry: Entry) {
        self.redo.clear();
        self.undo.push(entry);
        if self.undo.len() > MAX_ENTRIES {
            self.undo.remove(0);
        }
    }

    /// Undoes the last entry, returning its label; none when there is nothing
    /// to undo.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::GestureActive`] during a gesture.
    pub fn undo(&mut self, document: &mut Document) -> Result<Option<String>, AuthoringError> {
        self.reject_during_gesture()?;
        let Some(entry) = self.undo.pop() else {
            return Ok(None);
        };
        document.restore(&entry.before);
        let label = entry.label.clone();
        self.redo.push(entry);
        Ok(Some(label))
    }

    /// Redoes the last undone entry, returning its label; none when there is
    /// nothing to redo.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::GestureActive`] during a gesture.
    pub fn redo(&mut self, document: &mut Document) -> Result<Option<String>, AuthoringError> {
        self.reject_during_gesture()?;
        let Some(entry) = self.redo.pop() else {
            return Ok(None);
        };
        document.restore(&entry.after);
        let label = entry.label.clone();
        self.undo.push(entry);
        Ok(Some(label))
    }

    /// Starts a gesture, such as a drag: later previews change the document
    /// without recording anything until the gesture is committed.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::GestureActive`] when one is already active.
    pub fn begin_gesture(
        &mut self,
        document: &Document,
        label: impl Into<String>,
    ) -> Result<(), AuthoringError> {
        self.reject_during_gesture()?;
        self.gesture = Some(Gesture {
            label: label.into(),
            base: document.snapshot(),
        });
        Ok(())
    }

    /// Applies a command live during a gesture without recording an entry.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::NoGesture`] outside a gesture, or the command's
    /// rejection, which leaves the document as it was.
    pub fn preview(
        &mut self,
        document: &mut Document,
        command: &Command,
    ) -> Result<CommandOutcome, AuthoringError> {
        if self.gesture.is_none() {
            return Err(AuthoringError::NoGesture);
        }
        document.execute(command)
    }

    /// Ends the gesture and records one entry covering everything its previews
    /// changed, returning whether an entry was recorded. When nothing differs
    /// from where the gesture began, nothing is recorded and the document goes
    /// back to its state from before.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::NoGesture`] outside a gesture.
    pub fn commit_gesture(&mut self, document: &mut Document) -> Result<bool, AuthoringError> {
        let gesture = self.gesture.take().ok_or(AuthoringError::NoGesture)?;
        if document.data() == &gesture.base.data {
            if document.state() != gesture.base.state {
                document.restore(&gesture.base);
            }
            return Ok(false);
        }
        self.push(Entry {
            label: gesture.label,
            before: gesture.base,
            after: document.snapshot(),
            revision: document.revision(),
            key: None,
        });
        Ok(true)
    }

    /// Ends the gesture and restores the document to where it began.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::NoGesture`] outside a gesture.
    pub fn cancel_gesture(&mut self, document: &mut Document) -> Result<(), AuthoringError> {
        let gesture = self.gesture.take().ok_or(AuthoringError::NoGesture)?;
        document.restore(&gesture.base);
        Ok(())
    }

    /// Whether a gesture is in progress.
    pub fn in_gesture(&self) -> bool {
        self.gesture.is_some()
    }

    /// The label the active gesture will commit under.
    pub fn gesture_label(&self) -> Option<&str> {
        self.gesture.as_ref().map(|gesture| gesture.label.as_str())
    }

    /// Whether there is an entry to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is an entry to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The label of the entry undo would revert.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.label.as_str())
    }

    /// The label of the entry redo would reapply.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.label.as_str())
    }

    /// The labels of the undo stack, the next undo first.
    pub fn undo_labels(&self) -> Vec<&str> {
        self.undo
            .iter()
            .rev()
            .map(|entry| entry.label.as_str())
            .collect()
    }

    /// The labels of the redo stack, the next redo first.
    pub fn redo_labels(&self) -> Vec<&str> {
        self.redo
            .iter()
            .rev()
            .map(|entry| entry.label.as_str())
            .collect()
    }

    /// Drops every entry and any gesture; the document keeps its content.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.gesture = None;
    }

    /// Checks that no gesture is active.
    ///
    /// # Errors
    ///
    /// [`AuthoringError::GestureActive`] when one is.
    fn reject_during_gesture(&self) -> Result<(), AuthoringError> {
        if self.gesture.is_some() {
            Err(AuthoringError::GestureActive)
        } else {
            Ok(())
        }
    }
}
