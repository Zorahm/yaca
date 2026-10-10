//! Starting a command through the platform's shell.
//!
//! Which shell it is, is a fact the model has to know before it writes the
//! command, so it is stated in the system prompt next to the project root
//! rather than left to be discovered from a failure.
//!
//! On Windows the shell is PowerShell, not `cmd.exe`. cmd is what models write
//! worst: no quoting rules worth the name, `%VAR%`, `&` for `;`, and a command
//! of any size turns into a guess. PowerShell they write as readily as sh,
//! and it is on every Windows machine. PowerShell 7 (`pwsh`) is taken when it
//! is installed — it has `&&` and `||` — and Windows PowerShell 5.1 otherwise.
//!
//! How the command reaches it is the part worth reading; see
//! [`shell_command`].

use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::sync::mpsc;

/// No console window when the app spawns a command. Without it a black
/// rectangle flashes over the window on every call.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Which PowerShell commands run through.
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerShell {
    /// `powershell.exe`, 5.1: ships with Windows, so it is always there.
    Windows,
    /// `pwsh.exe`, 7 and later: installed separately.
    Core,
}

#[cfg(windows)]
impl PowerShell {
    fn exe(self) -> &'static str {
        match self {
            PowerShell::Windows => "powershell.exe",
            PowerShell::Core => "pwsh.exe",
        }
    }
}

/// The PowerShell of this machine, looked up once.
///
/// Once, because the system prompt names it, and the prompt is the cached
/// prefix of every request: an edition that changed mid-session because
/// someone installed `pwsh` would move the prefix and strand the reasoning
/// bound to it.
#[cfg(windows)]
pub fn powershell() -> PowerShell {
    static FOUND: std::sync::OnceLock<PowerShell> = std::sync::OnceLock::new();
    *FOUND.get_or_init(|| {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let installed = std::env::split_paths(&path)
            .any(|dir| dir.join(PowerShell::Core.exe()).is_file());
        if installed {
            PowerShell::Core
        } else {
            PowerShell::Windows
        }
    })
}

/// How the shell is named where a human reads it (§5.7 detail, system prompt).
#[cfg(windows)]
pub fn name() -> &'static str {
    powershell().exe()
}

#[cfg(unix)]
pub fn name() -> &'static str {
    "/bin/sh"
}

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
    let mut cmd = tokio::process::Command::from(shell_command(command)?);
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

/// What PowerShell is started with. Ours entirely — the command is not in
/// it — so there is nothing here to quote.
///
/// * Progress bars are off: with no console they arrive as XML on stderr.
/// * No colour. PowerShell 7 paints its errors with escape sequences, and the
///   feed would show `[31;1m` where the message starts. 5.1 has no `$PSStyle`
///   and nothing to turn off.
/// * Output is UTF-8. Left alone it is the machine's OEM codepage, and the
///   feed would show mojibake for every message in Russian (§0).
/// * The command comes out of the environment and is removed from it before
///   it runs, so what it starts does not inherit its own text.
/// * It runs as a script block. That is what makes errors plain text with
///   `At line:1` counted in the command itself.
#[cfg(windows)]
const BOOT: &str = "$ProgressPreference='SilentlyContinue';\
if($PSStyle){$PSStyle.OutputRendering='PlainText'};\
[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false);\
$OutputEncoding=[Console]::OutputEncoding;\
$c=$env:YACA_COMMAND;Remove-Item Env:YACA_COMMAND;\
& ([scriptblock]::Create($c))";

/// Appended to the command, on a line of its own.
///
/// PowerShell ends with 0 unless told otherwise, whatever the last program
/// returned, and a failed `cargo test` that reads as success is the feed
/// lying about what happened (§0). This makes it end the way sh does: with
/// the status of the last thing that ran. It has to sit inside the command's
/// own script — `$?` asked from outside the block is always true.
#[cfg(windows)]
const TAIL: &str = "\nif (-not $?) { if ($LASTEXITCODE) { exit $LASTEXITCODE }; exit 1 }\nexit 0";

/// The longest command taken. An environment variable holds 32 767
/// characters; this leaves room for [`TAIL`].
#[cfg(windows)]
const MAX_COMMAND_CHARS: usize = 30_000;

#[cfg(windows)]
const COMMAND_VAR: &str = "YACA_COMMAND";

/// The command goes through the environment, not the command line.
///
/// On the command line it would have to be quoted for two parsers in a row —
/// Windows' and PowerShell's — and every `"`, `$` and backtick in it is a
/// chance to run something other than what the human approved in §5.7. The
/// encoded form (`-EncodedCommand`) avoids the quoting and brings its own
/// trouble: errors come back as XML, with line numbers counted from our
/// preamble. An environment variable is taken literally, by both.
#[cfg(windows)]
fn shell_command(command: &str) -> std::io::Result<std::process::Command> {
    use std::os::windows::process::CommandExt;

    if command.encode_utf16().count() > MAX_COMMAND_CHARS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "команда длиннее {MAX_COMMAND_CHARS} символов — запишите её в файл .ps1 и запустите его"
            ),
        ));
    }

    let mut cmd = std::process::Command::new(powershell().exe());
    cmd.args([
        "-NoLogo",
        // The human's profile is theirs: it may print, prompt, or redefine
        // `cd`, and none of that was in the command they approved.
        "-NoProfile",
        "-NonInteractive",
        // The machine's policy on scripts is about scripts from elsewhere,
        // and on a stock Windows it also stops `npm` — which resolves to
        // `npm.ps1` — dead. What runs here was approved by the human (§5.7).
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        BOOT,
    ]);
    cmd.env(COMMAND_VAR, format!("{command}{TAIL}"));
    cmd.creation_flags(CREATE_NO_WINDOW);
    Ok(cmd)
}

#[cfg(unix)]
fn shell_command(command: &str) -> std::io::Result<std::process::Command> {
    use std::os::unix::process::CommandExt;

    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg("-c").arg(command);
    // Its own process group, so stopping it reaches what it started (kill.rs).
    cmd.process_group(0);
    Ok(cmd)
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::testutil::TempDir;

    /// Runs a command to the end: its exit code and everything it printed.
    async fn run(command: &str) -> (Option<i32>, String) {
        let dir = TempDir::new();
        let mut spawned = spawn(command, dir.path()).unwrap();
        let mut bytes = Vec::new();
        while let Some(chunk) = spawned.output.recv().await {
            bytes.extend(chunk);
        }
        let status = spawned.child.wait().await.unwrap();
        (status.code(), String::from_utf8_lossy(&bytes).into_owned())
    }

    #[tokio::test]
    async fn the_command_is_taken_literally() {
        // Each of these means something to cmd, to PowerShell's own command
        // line, or to both. None of them may be read before the command is.
        let (code, out) = run(r#"Write-Output "a `"b`" $(1 + 1) %PATH% ^ & | 'c'""#).await;
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(out.trim(), r#"a "b" 2 %PATH% ^ & | 'c'"#);
    }

    #[tokio::test]
    async fn a_failing_program_fails_the_command() {
        // PowerShell itself would end with 0 here.
        let (code, _) = run("cmd /c 'exit 7'").await;
        assert_eq!(code, Some(7));
    }

    #[tokio::test]
    async fn the_exit_code_is_the_last_commands_as_in_sh() {
        let (code, out) = run("cmd /c 'exit 7'; Write-Output after").await;
        assert_eq!(code, Some(0));
        assert_eq!(out.trim(), "after");
    }

    #[tokio::test]
    async fn a_cmdlet_that_fails_last_fails_the_command() {
        let (code, out) = run("Get-Item no-such-file-here").await;
        assert_eq!(code, Some(1));
        // Plain text, whichever PowerShell it is: 5.1 would send XML if the
        // command arrived encoded, and 7 paints its errors unless told not to.
        assert!(out.contains("no-such-file-here"), "{out}");
        assert!(!out.contains("CLIXML"), "{out}");
        assert!(!out.contains('\u{1b}'), "{out:?}");
    }

    #[tokio::test]
    async fn chaining_works_the_way_the_prompt_says_it_does() {
        // The system prompt tells the model which of the two it may write.
        let chained = match powershell() {
            PowerShell::Core => "Write-Output a && Write-Output b",
            PowerShell::Windows => "Write-Output a; if ($?) { Write-Output b }",
        };
        let (code, out) = run(chained).await;
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(out.split_whitespace().collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[tokio::test]
    async fn an_explicit_exit_is_kept() {
        let (code, out) = run("Write-Output before; exit 3").await;
        assert_eq!(code, Some(3));
        assert_eq!(out.trim(), "before");
    }

    #[tokio::test]
    async fn russian_comes_out_as_utf8() {
        let (_, out) = run("Write-Output 'привет'").await;
        assert_eq!(out.trim(), "привет");
    }

    #[tokio::test]
    async fn several_lines_are_one_script() {
        let (code, out) = run("$x = 2\n$y = 3\nWrite-Output ($x * $y)").await;
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(out.trim(), "6");
    }

    #[tokio::test]
    async fn what_the_command_starts_does_not_inherit_its_text() {
        let (_, out) = run("cmd /c 'echo [%YACA_COMMAND%]'").await;
        // cmd leaves an unset variable as written.
        assert_eq!(out.trim(), "[%YACA_COMMAND%]");
    }

    #[tokio::test]
    async fn a_command_too_long_to_carry_is_refused_before_it_starts() {
        let dir = TempDir::new();
        let long = format!("Write-Output '{}'", "x".repeat(MAX_COMMAND_CHARS));
        let err = spawn(&long, dir.path()).err().expect("should not start");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
