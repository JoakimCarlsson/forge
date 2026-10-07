//! One running game process with its output.

use std::process::{Child, ExitStatus};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use fr_authoring::LaunchCommand;

use super::output::RunEvent;
use super::pipe::{Message, STREAM_COUNT, kill_group, spawn_piped};

/// How long to wait for the last output after a process has ended.
const OUTPUT_GRACE: Duration = Duration::from_millis(250);

/// A child process started from a [`LaunchCommand`], its standard output and
/// error drained by reader threads so reading never blocks the editor.
pub struct GameProcess {
    /// The child.
    child: Child,
    /// The lines the reader threads send.
    receiver: Receiver<Message>,
    /// How many of the child's streams have not reached their end.
    open_streams: usize,
    /// Whether the end of the process has been reported.
    finished: bool,
}

impl GameProcess {
    /// Starts the command.
    ///
    /// # Errors
    ///
    /// The reason, as text, when the process cannot be started.
    pub fn spawn(command: &LaunchCommand) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        let child = spawn_piped(command.to_command(), &sender)
            .map_err(|error| format!("cannot start {}: {error}", command.program.display()))?;
        Ok(Self {
            child,
            receiver,
            open_streams: STREAM_COUNT,
            finished: false,
        })
    }

    /// Whether the process has not been reported as ended.
    pub fn is_running(&self) -> bool {
        !self.finished
    }

    /// The next message: waiting a short while for streams still open when
    /// `wait` is set, otherwise only what is already there.
    fn next_message(&self, wait: bool) -> Option<Message> {
        if wait && self.open_streams > 0 {
            self.receiver.recv_timeout(OUTPUT_GRACE).ok()
        } else {
            self.receiver.try_recv().ok()
        }
    }

    /// The output lines received so far.
    fn collect(&mut self, wait: bool) -> Vec<RunEvent> {
        let mut events = Vec::new();
        while let Some(message) = self.next_message(wait) {
            match message {
                Message::Line(line) => events.push(RunEvent::Output(line)),
                Message::Closed => self.open_streams = self.open_streams.saturating_sub(1),
            }
        }
        events
    }

    /// The last output and the exit event of an ended process.
    fn finish(&mut self, status: ExitStatus) -> Vec<RunEvent> {
        let mut events = self.collect(true);
        events.push(RunEvent::Exited {
            code: status.code(),
            success: status.success(),
        });
        self.finished = true;
        events
    }

    /// The output since the last call, followed by the exit event once the
    /// process has ended. Never blocks for longer than the short grace period
    /// after an exit.
    pub fn poll(&mut self) -> Vec<RunEvent> {
        if self.finished {
            return self.collect(false);
        }
        match self.child.try_wait() {
            Ok(Some(status)) => self.finish(status),
            Ok(None) => self.collect(false),
            Err(_) => {
                let mut events = self.collect(false);
                events.push(RunEvent::Exited {
                    code: None,
                    success: false,
                });
                self.finished = true;
                events
            }
        }
    }

    /// Kills the process and its process group, waits for it and returns the
    /// remaining output and the exit event; empty when it had already ended.
    pub fn stop(&mut self) -> Vec<RunEvent> {
        if self.finished {
            return self.collect(false);
        }
        kill_group(&mut self.child);
        match self.child.wait() {
            Ok(status) => self.finish(status),
            Err(_) => {
                self.finished = true;
                vec![RunEvent::Exited {
                    code: None,
                    success: false,
                }]
            }
        }
    }
}

impl Drop for GameProcess {
    /// Kills a process still running, so closing the editor never leaves a game
    /// behind.
    fn drop(&mut self) {
        if !self.finished {
            kill_group(&mut self.child);
            let _ = self.child.wait();
        }
    }
}
