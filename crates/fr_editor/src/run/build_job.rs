//! The build steps of a project, run one after another on a worker thread.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use fr_project::BuildStep;

use super::output::{OutputLine, RunEvent, notice};
use super::pipe::{Message, STREAM_COUNT, kill_group, spawn_piped};

/// How often a running step checks whether the build was cancelled.
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// What the worker thread sends.
enum JobMessage {
    /// A line of a step's output or an editor message.
    Output(OutputLine),
    /// The last message: whether every step succeeded.
    Finished(bool),
}

/// A build running on a worker thread. Dropping it cancels it.
pub struct BuildJob {
    /// What the worker sends.
    receiver: Receiver<JobMessage>,
    /// Set to stop the worker and kill the step in progress.
    cancel: Arc<AtomicBool>,
    /// Whether the end of the build has been reported.
    finished: bool,
}

/// Sends an editor message to the job's receiver.
fn say(sender: &Sender<JobMessage>, text: &str) {
    if let RunEvent::Output(line) = notice(text) {
        let _ = sender.send(JobMessage::Output(line));
    }
}

/// Runs one step, forwarding its output, and whether it succeeded. A cancelled
/// step is killed and counts as failed.
fn run_step(step: &BuildStep, sender: &Sender<JobMessage>, cancel: &AtomicBool) -> bool {
    say(sender, &format!("{}...", step.description));
    let mut command = Command::new(&step.executable);
    command
        .args(&step.arguments)
        .current_dir(&step.working_directory);
    let (lines, receiver) = mpsc::channel();
    let mut child = match spawn_piped(command, &lines) {
        Ok(child) => child,
        Err(error) => {
            say(
                sender,
                &format!("cannot start {}: {error}", step.executable),
            );
            return false;
        }
    };
    drop(lines);
    let mut open_streams = STREAM_COUNT;
    while open_streams > 0 {
        if cancel.load(Ordering::Relaxed) {
            kill_group(&mut child);
            let _ = child.wait();
            return false;
        }
        match receiver.recv_timeout(CANCEL_POLL) {
            Ok(Message::Line(line)) => {
                let _ = sender.send(JobMessage::Output(line));
            }
            Ok(Message::Closed) => open_streams -= 1,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    match child.wait() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            say(sender, &format!("{} exited with {status}", step.executable));
            false
        }
        Err(error) => {
            say(
                sender,
                &format!("cannot wait for {}: {error}", step.executable),
            );
            false
        }
    }
}

/// Runs the steps in order, stopping at the first failure, and sends the result.
fn run_steps(steps: &[BuildStep], sender: &Sender<JobMessage>, cancel: &AtomicBool) {
    let ok = steps.iter().all(|step| run_step(step, sender, cancel));
    let _ = sender.send(JobMessage::Finished(ok));
}

impl BuildJob {
    /// Starts the steps on a worker thread.
    pub fn start(steps: Vec<BuildStep>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        thread::spawn(move || run_steps(&steps, &sender, &worker_cancel));
        Self {
            receiver,
            cancel,
            finished: false,
        }
    }

    /// Whether the end of the build has been reported.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Stops the build: the step in progress is killed and no later step runs.
    /// The end is still reported by [`poll`](Self::poll), as a failure.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The output since the last call, ending with
    /// [`RunEvent::BuildFinished`] once the build is over. Never blocks.
    pub fn poll(&mut self) -> Vec<RunEvent> {
        let mut events = Vec::new();
        while !self.finished {
            match self.receiver.try_recv() {
                Ok(JobMessage::Output(line)) => events.push(RunEvent::Output(line)),
                Ok(JobMessage::Finished(ok)) => {
                    self.finished = true;
                    events.push(RunEvent::BuildFinished { ok });
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.finished = true;
                    events.push(RunEvent::BuildFinished { ok: false });
                }
            }
        }
        events
    }
}

impl Drop for BuildJob {
    /// Cancels a build still running.
    fn drop(&mut self) {
        self.cancel();
    }
}
