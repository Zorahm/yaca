//! Commands that run in the background.
//!
//! Two roads lead here: the model asks for `background: true`, or a foreground
//! command outlives its wait and is moved. Either way the command ends up in
//! the process panel (§6.5), and its output is collected here until it ends.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Child;
use tokio::sync::mpsc;

use super::bash::{head, MODEL_OUTPUT_MAX_BYTES};
use super::ToolOutput;
use super::ToolCtx;
use crate::error::ToolError;
use crate::process::output::{clamp, Lines, Ring, Utf8Stream};
use crate::process::{display_cwd_in, shell, ProcessId, WeakProcesses};

/// Past this the file of a background command's output stops growing. A
/// runaway command printing in a loop would otherwise fill the disk.
const OUTPUT_FILE_MAX_BYTES: u64 = 20 * 1024 * 1024;

/// What a foreground command was waited for with, for the message that says
/// the wait is over.
pub(super) struct Waited<'a> {
    pub(super) command: &'a str,
    pub(super) cwd: &'a Path,
    pub(super) limit: Duration,
    pub(super) pid: Option<u32>,
}

/// The wait ran out with the command still going: it takes its place in the
/// process panel and keeps running.
///
/// Killing it would throw away what may be ten minutes of a build, and the
/// model would only start it again with `background`. What it has printed goes
/// to the model now; what it prints from here goes to its buffer and to a
/// file, and the model is told when it ends and where the file is
/// (`Worker::process_note`).
pub(super) async fn to_background(
    ctx: &ToolCtx,
    waited: Waited<'_>,
    child: Child,
    output: mpsc::Receiver<Vec<u8>>,
    ring: Ring,
    bytes: Utf8Stream,
    lines: Lines,
) -> ToolOutput {
    let printed: Vec<String> = ring.lines().into_iter().map(str::to_owned).collect();
    let produced = ring.next_index();
    let (text, truncated) = clamp(&ring.lines(), MODEL_OUTPUT_MAX_BYTES);

    let id = ctx.processes.adopt(
        waited.command.to_string(),
        display_cwd_in(&ctx.ws, waited.cwd),
        waited.pid,
        printed.clone(),
    );
    let file = OutputFile::create(ctx.ws.home(), id, &printed).await;
    if let Some(file) = &file {
        ctx.processes.set_output_file(id, file.path.clone());
    }
    tokio::spawn(collect_from(
        ctx.processes.downgrade(),
        id,
        child,
        output,
        bytes,
        lines,
        file,
    ));

    let secs = waited.limit.as_secs();
    ctx.progress.push(format!(
        "Не уложилась в {secs} с — продолжает работать в фоне под номером {id}\n"
    ));

    let printed_so_far = if text.is_empty() {
        "[no output yet]\n".to_string()
    } else {
        text
    };
    ToolOutput::new(format!(
        "{printed_so_far}\n[Still running after {secs} s, so it was NOT stopped: it moved \
         to the background as command {id}. Do not start it again. Read what it prints \
         from now on with BashOutput, stop it with BashKill. When it ends you will be \
         told, with the path of a file holding its whole output.]\n"
    ))
    .lines(produced.min(u32::MAX as u64) as u32)
    .truncated(truncated)
}

/// The whole output of a command that moved to the background, on disk.
///
/// The buffer keeps ten thousand lines; a build that ran for ten minutes has
/// more, and the failure at the bottom may need the first error at the top.
struct OutputFile {
    path: PathBuf,
    file: tokio::fs::File,
    written: u64,
}

impl OutputFile {
    /// `None` when the file cannot be made: the command then has no file, and
    /// the buffer is still there for `BashOutput`.
    async fn create(root: &Path, id: ProcessId, printed: &[String]) -> Option<OutputFile> {
        let dir = crate::project::dir(root).join("background");
        tokio::fs::create_dir_all(&dir).await.ok()?;
        // The number alone repeats across sessions, and two windows on one
        // project would write to the same file.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let path = dir.join(format!("command-{id}-{stamp}.log"));
        let file = tokio::fs::File::create(&path).await.ok()?;
        let mut made = OutputFile {
            path,
            file,
            written: 0,
        };
        made.write(printed).await;
        Some(made)
    }

    async fn write(&mut self, lines: &[String]) {
        use tokio::io::AsyncWriteExt;

        if self.written > OUTPUT_FILE_MAX_BYTES {
            return;
        }
        let mut chunk = String::new();
        for line in lines {
            chunk.push_str(line);
            chunk.push('\n');
        }
        self.written += chunk.len() as u64;
        if self.written > OUTPUT_FILE_MAX_BYTES {
            chunk.push_str("[вывод обрезан: файл достиг предела размера]\n");
        }
        // A full disk is no reason to stop collecting into the buffer.
        let _ = self.file.write_all(chunk.as_bytes()).await;
    }
}

/// Starts the command and leaves it in the process panel (§6.5).
pub(super) fn background(ctx: &ToolCtx, command: &str, cwd: &Path) -> Result<ToolOutput, ToolError> {
    let spawned = shell::spawn(ctx.shell, command, cwd)
        .map_err(|e| ToolError::failed(format!("не удалось запустить «{}»: {e}", head(command))))?;
    let shell::Spawned { child, pid, output } = spawned;

    let id = ctx
        .processes
        .register(command.to_string(), display_cwd_in(&ctx.ws, cwd), pid);
    tokio::spawn(collect(ctx.processes.downgrade(), id, child, output));

    Ok(ToolOutput::new(format!(
        "Запущена в фоне под номером {id}. Читать её вывод — BashOutput, \
         остановить — BashKill, перезапустить — BashRestart.\n"
    ))
    .lines(1))
}

/// Feeds a background command's output into its ring buffer until it ends.
///
/// Holds the registry weakly on purpose. A strong handle would keep the
/// session's processes alive for as long as `npm run dev` runs — that is,
/// forever — and the drop that kills them at the end of a session would never
/// fire. Here the session going away closes the pipes instead, and this task
/// notices and leaves.
pub(super) async fn collect(
    processes: WeakProcesses,
    id: ProcessId,
    child: Child,
    output: mpsc::Receiver<Vec<u8>>,
) {
    collect_from(
        processes,
        id,
        child,
        output,
        Utf8Stream::default(),
        Lines::default(),
        None,
    )
    .await;
}

/// [`collect`] for a command that already ran in the foreground: the half
/// line it had printed when it moved is finished here rather than split, and
/// `file` gets every line as well as the buffer.
async fn collect_from(
    processes: WeakProcesses,
    id: ProcessId,
    mut child: Child,
    mut output: mpsc::Receiver<Vec<u8>>,
    mut bytes: Utf8Stream,
    mut lines: Lines,
    mut file: Option<OutputFile>,
) {
    while let Some(raw) = output.recv().await {
        let text = bytes.push(&raw);
        let Some(processes) = processes.upgrade() else {
            return;
        };
        let complete = lines.push(&text);
        if let Some(file) = file.as_mut() {
            file.write(&complete).await;
        }
        processes.output(id, complete);
    }

    let mut last = lines.push(&bytes.flush());
    last.extend(lines.flush());
    if let Some(file) = file.as_mut() {
        file.write(&last).await;
    }
    let code = child.wait().await.ok().and_then(|s| s.code());

    if let Some(processes) = processes.upgrade() {
        processes.output(id, last);
        processes.finished(id, code);
    }
}

