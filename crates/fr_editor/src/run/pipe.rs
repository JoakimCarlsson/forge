//! Spawning a child with piped output that reader threads drain into a channel.

use std::io::{self, BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

use super::output::{OutputLine, Stream};

/// How many output streams a child has: standard output and standard error.
pub(super) const STREAM_COUNT: usize = 2;

/// What a reader thread sends.
pub(super) enum Message {
    /// A line was read.
    Line(OutputLine),
    /// A stream reached its end.
    Closed,
}

/// Puts the child in a process group of its own on unix, so the whole group
/// can be killed and `cargo run` children die with it.
#[cfg(unix)]
fn isolate_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// Does nothing where process groups do not exist.
#[cfg(not(unix))]
fn isolate_process_group(_command: &mut Command) {}

/// Reads a stream line by line until its end, sending each line and then
/// [`Message::Closed`]. Invalid UTF-8 is replaced, never an error.
fn pump(reader: impl Read, stream: Stream, sender: &Sender<Message>) {
    let mut reader = BufReader::new(reader);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buffer)
                    .trim_end_matches(['\n', '\r'])
                    .to_owned();
                if sender
                    .send(Message::Line(OutputLine { stream, text }))
                    .is_err()
                {
                    return;
                }
            }
        }
    }
    let _ = sender.send(Message::Closed);
}

/// Starts a thread that pumps one stream into the sender.
fn pump_in_thread(reader: impl Read + Send + 'static, stream: Stream, sender: Sender<Message>) {
    thread::spawn(move || pump(reader, stream, &sender));
}

/// Spawns the command with piped output and no input, in its own process
/// group, with reader threads sending its lines to the sender.
///
/// # Errors
///
/// When the process cannot be started.
pub(super) fn spawn_piped(mut command: Command, sender: &Sender<Message>) -> io::Result<Child> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    isolate_process_group(&mut command);
    let mut child = command.spawn()?;
    if let Some(stdout) = child.stdout.take() {
        pump_in_thread(stdout, Stream::Stdout, sender.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        pump_in_thread(stderr, Stream::Stderr, sender.clone());
    }
    Ok(child)
}

/// Kills the child and, on unix, its whole process group.
///
/// The group is killed by running `kill -KILL -- -<pid>`, which needs no
/// `libc`; the child is then killed directly as well, which also covers the
/// case where the `kill` program is missing. Failures are ignored because the
/// process may already be gone.
pub(super) fn kill_group(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}
