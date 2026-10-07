//! The failures of reading and writing authored files.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Why an authored file could not be read, written or understood.
#[derive(Debug)]
pub enum DocumentError {
    /// The file system refused an operation on a path.
    Io {
        /// The file or directory concerned.
        path: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },
    /// The text is not valid JSON.
    Syntax {
        /// What the parser reported, with the line and column.
        message: String,
    },
    /// The file is not UTF-8 text.
    NotUtf8 {
        /// The offset of the first byte that is not valid.
        offset: usize,
    },
    /// The file is JSON of another format than the one asked for.
    WrongFormat {
        /// The format that was required.
        required: &'static str,
        /// The format the file names, empty when it names none.
        found: String,
    },
    /// The file is of the right format at a version this build does not read.
    Version {
        /// The format whose version differs.
        format: String,
        /// The version the file carries.
        found: u32,
        /// The only version this build reads.
        required: u32,
    },
    /// The file is valid JSON that does not describe the data it should.
    Malformed {
        /// What is wrong.
        message: String,
    },
    /// Another failure, located in a file.
    InFile {
        /// The file concerned.
        path: PathBuf,
        /// What went wrong in it.
        source: Box<DocumentError>,
    },
}

impl DocumentError {
    /// A failure of the file system on `path`.
    pub fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    /// A description of data that is not what it should be.
    pub fn malformed(message: impl Into<String>) -> Self {
        Self::Malformed {
            message: message.into(),
        }
    }

    /// The same failure located in `path`; an error already in a file is kept.
    pub fn in_file(self, path: &Path) -> Self {
        match self {
            Self::Io { .. } | Self::InFile { .. } => self,
            other => Self::InFile {
                path: path.to_path_buf(),
                source: Box::new(other),
            },
        }
    }
}

impl fmt::Display for DocumentError {
    /// Writes the cause; a version mismatch names the version found and required.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Syntax { message } => write!(f, "invalid JSON: {message}"),
            Self::NotUtf8 { offset } => write!(f, "not UTF-8 text at byte {offset}"),
            Self::WrongFormat { required, found } if found.is_empty() => {
                write!(f, "not a {required} document")
            }
            Self::WrongFormat { required, found } => {
                write!(f, "not a {required} document (found \"{found}\")")
            }
            Self::Version {
                format,
                found,
                required,
            } => write!(
                f,
                "{format} format version {found} is not supported; version {required} is required"
            ),
            Self::Malformed { message } => f.write_str(message),
            Self::InFile { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl Error for DocumentError {
    /// The wrapped failure, when there is one.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InFile { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
