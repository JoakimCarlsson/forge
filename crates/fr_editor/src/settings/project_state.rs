//! The editor's state for one project, kept in the project's `.forge-editor`
//! folder and ignored by version control.

use std::path::{Path, PathBuf};

use super::ini::{all_values, first_value, format_ini, parse_ini, read_optional, write_text};

/// The folder in a project that holds the editor's state.
pub const PROJECT_STATE_DIRECTORY: &str = ".forge-editor";

/// The state file in that folder.
const STATE_FILE: &str = "state.ini";

/// The ignore file that keeps the folder out of version control.
const IGNORE_FILE: &str = ".gitignore";

/// What the ignore file holds: everything in the folder, itself included.
const IGNORE_TEXT: &str = "*\n";

/// The documents that were open in a project and the one in front.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectState {
    /// The open documents as paths relative to the asset root, in tab order.
    pub open_documents: Vec<String>,
    /// The document in front, the last open scene.
    pub current: Option<String>,
}

impl ProjectState {
    /// The folder of a project's editor state.
    pub fn directory(project: &Path) -> PathBuf {
        project.join(PROJECT_STATE_DIRECTORY)
    }

    /// Loads the state of a project; a missing file gives the empty state.
    ///
    /// # Errors
    ///
    /// The reason, as text, when the file exists and cannot be read.
    pub fn load(project: &Path) -> Result<Self, String> {
        let path = Self::directory(project).join(STATE_FILE);
        let Some(text) = read_optional(&path)? else {
            return Ok(Self::default());
        };
        let entries = parse_ini(&text);
        Ok(Self {
            open_documents: all_values(&entries, "open_document")
                .map(str::to_owned)
                .collect(),
            current: first_value(&entries, "current")
                .filter(|current| !current.is_empty())
                .map(str::to_owned),
        })
    }

    /// Writes the state atomically, creating the folder and its `.gitignore`
    /// when needed.
    ///
    /// # Errors
    ///
    /// The reason, as text, when a file cannot be written.
    pub fn save(&self, project: &Path) -> Result<(), String> {
        let directory = Self::directory(project);
        let ignore = directory.join(IGNORE_FILE);
        if !ignore.is_file() {
            write_text(&ignore, IGNORE_TEXT)?;
        }
        let mut entries: Vec<(&str, String)> = self
            .open_documents
            .iter()
            .map(|document| ("open_document", document.clone()))
            .collect();
        if let Some(current) = &self.current {
            entries.push(("current", current.clone()));
        }
        write_text(&directory.join(STATE_FILE), &format_ini(entries))
    }
}
