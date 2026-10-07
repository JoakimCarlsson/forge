//! The question asked before something would lose unsaved changes.

use std::path::PathBuf;

use fr_authoring::DocumentId;

/// What waits for the answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    /// Closing one document.
    CloseDocument(DocumentId),
    /// Closing the project.
    CloseProject,
    /// Ending the editor.
    Quit,
    /// Opening another project.
    OpenProject(PathBuf),
}

/// An open question: what is at stake and what happens on the answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    /// The question.
    pub text: String,
    /// What is done when the user has answered.
    pub then: Pending,
}
