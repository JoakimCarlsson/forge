//! The lines and events a build or a running game produces.

/// The prefix of the editor's own messages, like the engine's `forge:`.
pub const MESSAGE_PREFIX: &str = "forge: ";

/// Where a line of output came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    /// The child's standard output.
    Stdout,
    /// The child's standard error.
    Stderr,
    /// The editor itself, not a child.
    Editor,
}

/// One line of text from a build step, the game or the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputLine {
    /// Where the line came from.
    pub stream: Stream,
    /// The line without its line ending.
    pub text: String,
}

/// Something that happened during a build or a run, in the order it happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEvent {
    /// A line of output.
    Output(OutputLine),
    /// The game process ended.
    Exited {
        /// The exit code; none when a signal ended the process.
        code: Option<i32>,
        /// Whether the process reported success.
        success: bool,
    },
    /// The build ended.
    BuildFinished {
        /// Whether every step succeeded.
        ok: bool,
    },
}

/// An editor message as an output event, prefixed with [`MESSAGE_PREFIX`].
pub fn notice(text: impl AsRef<str>) -> RunEvent {
    RunEvent::Output(OutputLine {
        stream: Stream::Editor,
        text: format!("{MESSAGE_PREFIX}{}", text.as_ref()),
    })
}
