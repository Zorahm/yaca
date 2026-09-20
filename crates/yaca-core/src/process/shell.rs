//! Starting a command through the platform's shell.
//!
//! Which shell it is, is a fact the model has to know before it writes the
//! command, so it is stated in the system prompt next to the project root
//! rather than left to be discovered from a failure.

use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::sync::mpsc;

/// No console window when the app spawns a command. Without it a black
/// rectangle flashes over the window on every call.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// How the shell is named where a human reads it (§5.7 detail, system prompt).
#[cfg(windows)]
pub const NAME: &str = "cmd.exe";
#[cfg(unix)]
pub const NAME: &str = "/bin/sh";

/// One read of a pipe. Not one line: the kernel hands over whatever it has, and
/// batching that way keeps a noisy build from becoming one event per line.
const READ_BYTES: usize = 8 * 1024;

pub struct Spawned {
    pub child: Child,
    /// `None` only if the child ended between spawn and this call.
    pub pid: Option<u32>,
    /// stdout and stderr merged, the way a terminal shows them. Closes when
    /// both pipes reach EOF, which is the signal that the output is complete.
    pub output: mpsc::Receiver<Vec<u8>>,
}

pub fn spawn(command: &str, cwd: &Path) -> std::io::Result<Spawned> {
    let mut cmd = tokio::process::Command::from(shell_command(command));
    cmd.current_dir(cwd)
        // No terminal is attached. Anything that stops to ask a question would
        // otherwise wait for an answer that cannot arrive.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The runtime shutting down must not leave commands behind.
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;
    let pid = child.id();

    let (tx, output) = mpsc::channel(64);
    if let Some(out) = child.stdout.take() {
        tokio::spawn(pump(out, tx.clone()));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(pump(err, tx));
    }

    Ok(Spawned { child, pid, output })
}

#[cfg(windows)]
fn shell_command(command: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;

    let mut cmd = std::process::Command::new("cmd.exe");
    // `/D` skips the machine's AutoRun entries: nothing out of the registry
    // rides along with the command the human approved in §5.7. `/S` together
    // with the outer quotes tells cmd to strip exactly those and take the rest
    // literally, which is the only way `&&`, `>` and inner quotes survive.
    //
    // `raw_arg`, not `arg`: since the 2024 batch-file fix std escapes arguments
    // it is about to hand to cmd, and that escaping mangles every operator.
    cmd.raw_arg(format!("/D /S /C \"{command}\""));
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(unix)]
fn shell_command(command: &str) -> std::process::Command {
    use std::os::unix::process::CommandExt;

    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg("-c").arg(command);
    // Its own process group, so stopping it reaches what it started (kill.rs).
    cmd.process_group(0);
    cmd
}

async fn pump<R: AsyncRead + Unpin + Send + 'static>(mut src: R, tx: mpsc::Sender<Vec<u8>>) {
    let mut buf = vec![0u8; READ_BYTES];
    loop {
        match src.read(&mut buf).await {
            // EOF, or a pipe that broke because the child was killed.
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if tx.send(buf[..n].to_vec()).await.is_err() {
                    return;
                }
            }
        }
    }
}
