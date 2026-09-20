//! Ending a command means ending everything it started.
//!
//! `Child::kill` reaches the shell and stops there. `cmd /C npm test` would
//! leave node running, the panel would go on showing a process that is gone
//! while the one doing the work stayed — and "Stop" (§6.5) would be a button
//! that sometimes works, which §0 calls an interface lying about state.
//!
//! Force rather than a polite signal, on both platforms. A process that decides
//! to ignore the request would sit in the panel as `running` for the rest of
//! the session, and there is no second button behind this one.

/// Kills the process and everything it spawned. Blocking: on Windows this runs
/// `taskkill`, which is a process of its own.
#[cfg(windows)]
pub fn tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(super::shell::CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(unix)]
pub fn tree(pid: u32) {
    // The child leads its own process group — `process_group(0)` at spawn —
    // so a negative pid reaches every descendant in one call.
    let Ok(group) = i32::try_from(pid) else {
        return;
    };
    // SAFETY: `kill` takes two integers, touches no memory we own, and a stale
    // pid at worst returns ESRCH.
    unsafe {
        libc::kill(-group, libc::SIGKILL);
    }
}
