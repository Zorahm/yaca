//! Raw bytes from a child's pipes turned into lines, and the last 10 000 of
//! them kept (§6.5).
//!
//! Three small pieces rather than one, because they fail for different reasons:
//! [`Utf8Stream`] fixes reads that end mid-character, [`Lines`] decides what a
//! line even is when a progress bar keeps rewriting one, and [`Ring`] is the
//! bound on memory.

use std::collections::VecDeque;

/// §6.5 fixes the buffer at 10 000 lines.
pub const RING_LINES: usize = 10_000;

/// Past this a "line" is not a line. A command that prints without newlines —
/// a spinner, a `dd` writing to stdout — would otherwise grow the pending
/// buffer without bound, and the feed would show nothing until it finished.
const MAX_LINE_BYTES: usize = 8 * 1024;

/// Bytes of a pipe turned into text, one `read` at a time.
///
/// A read ends wherever the kernel had bytes ready, which is regularly in the
/// middle of a character. Decoding each read on its own would put a replacement
/// character into every few kilobytes of Cyrillic output; the unfinished tail
/// waits for the rest instead.
#[derive(Default)]
pub struct Utf8Stream {
    tail: Vec<u8>,
}

impl Utf8Stream {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        let mut buf = std::mem::take(&mut self.tail);
        buf.extend_from_slice(bytes);

        let mut out = String::new();
        let mut at = 0usize;
        loop {
            match std::str::from_utf8(&buf[at..]) {
                Ok(rest) => {
                    out.push_str(rest);
                    at = buf.len();
                    break;
                }
                Err(err) => {
                    let good = err.valid_up_to();
                    // A prefix `valid_up_to` reported is valid by definition.
                    out.push_str(std::str::from_utf8(&buf[at..at + good]).unwrap_or_default());
                    at += good;
                    match err.error_len() {
                        // Cut in the middle of a character: wait for the rest.
                        None => break,
                        // Not UTF-8 at all. On Windows that is the usual case
                        // rather than corruption, so the whole buffer is
                        // reread in the console encoding; elsewhere broken
                        // bytes are broken bytes and never complete.
                        Some(bad) => {
                            if let Some(text) = legacy::decode(&buf) {
                                self.tail.clear();
                                return text;
                            }
                            out.push(char::REPLACEMENT_CHARACTER);
                            at += bad;
                        }
                    }
                }
            }
        }

        self.tail = buf[at..].to_vec();
        out
    }

    /// The pipe closed. Whatever is left will never be completed.
    pub fn flush(&mut self) -> String {
        if self.tail.is_empty() {
            return String::new();
        }
        let tail = std::mem::take(&mut self.tail);
        legacy::decode(&tail).unwrap_or_else(|| String::from_utf8_lossy(&tail).into_owned())
    }
}

/// Output that is not UTF-8, read the way the platform meant it.
///
/// Older Windows tools, and `cmd` when a command calls it, write in the
/// machine's OEM codepage —
/// Russian text comes out as eight cp866 bytes — and showing that as replacement characters
/// would be the feed lying about what the command printed (§0). Prefixing the
/// command with `chcp 65001` was tried and dropped: it changes the line the
/// human approved in §5.7, and it worked only sometimes, because cmd settles
/// its output codepage before the prefix has run.
///
/// UTF-8 is always tried first, so a modern tool is never reinterpreted.
mod legacy {
    /// `None` where there is nothing better to try than replacement characters.
    #[cfg(not(windows))]
    pub fn decode(_bytes: &[u8]) -> Option<String> {
        None
    }

    #[cfg(windows)]
    pub fn decode(bytes: &[u8]) -> Option<String> {
        use windows_sys::Win32::Globalization::{GetOEMCP, MultiByteToWideChar};

        if bytes.is_empty() || bytes.len() > i32::MAX as usize {
            return None;
        }
        let codepage = unsafe { GetOEMCP() };
        let len = bytes.len() as i32;

        // SAFETY: both calls get a pointer and a length describing the same
        // buffer, and the second is given exactly the size the first asked for.
        let wide_len = unsafe {
            MultiByteToWideChar(codepage, 0, bytes.as_ptr(), len, std::ptr::null_mut(), 0)
        };
        if wide_len <= 0 {
            return None;
        }
        let mut wide = vec![0u16; wide_len as usize];
        let written = unsafe {
            MultiByteToWideChar(
                codepage,
                0,
                bytes.as_ptr(),
                len,
                wide.as_mut_ptr(),
                wide_len,
            )
        };
        if written <= 0 {
            return None;
        }
        wide.truncate(written as usize);
        Some(String::from_utf16_lossy(&wide))
    }
}

/// Text cut into lines.
///
/// `\r\n` is one terminator. A lone `\r` is one too: that is a progress bar
/// starting its line over, and keeping the whole thing as a single line would
/// mean one 200-kilobyte line for a build.
#[derive(Default)]
pub struct Lines {
    pending: String,
    after_cr: bool,
}

impl Lines {
    pub fn push(&mut self, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for ch in text.chars() {
            match ch {
                '\r' => {
                    out.push(std::mem::take(&mut self.pending));
                    self.after_cr = true;
                }
                // The `\n` half of a `\r\n`: the line already ended.
                '\n' if self.after_cr => self.after_cr = false,
                '\n' => out.push(std::mem::take(&mut self.pending)),
                _ => {
                    self.after_cr = false;
                    self.pending.push(ch);
                    if self.pending.len() >= MAX_LINE_BYTES {
                        out.push(std::mem::take(&mut self.pending));
                    }
                }
            }
        }
        out
    }

    /// The stream ended. A last line without a terminator is still a line.
    pub fn flush(&mut self) -> Option<String> {
        self.after_cr = false;
        (!self.pending.is_empty()).then(|| std::mem::take(&mut self.pending))
    }
}

/// The last `cap` lines of a command's output.
pub struct Ring {
    lines: VecDeque<String>,
    dropped: u64,
    cap: usize,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Ring {
            lines: VecDeque::new(),
            dropped: 0,
            cap: cap.max(1),
        }
    }

    pub fn push(&mut self, line: String) {
        if self.lines.len() == self.cap {
            self.lines.pop_front();
            self.dropped += 1;
        }
        self.lines.push_back(line);
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Lines that fell out of the buffer. Shown rather than hidden: output the
    /// panel silently lost looks exactly like output the command never printed.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Index one past the newest line, counted from the command's first line
    /// ever — not from the start of the buffer, which moves.
    pub fn next_index(&self) -> u64 {
        self.dropped + self.lines.len() as u64
    }

    pub fn lines(&self) -> Vec<&str> {
        self.lines.iter().map(String::as_str).collect()
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// Lines with an absolute index at or after `from`, and how many were
    /// missed because the buffer had already dropped them.
    pub fn since(&self, from: u64) -> (Vec<&str>, u64) {
        let missed = self.dropped.saturating_sub(from);
        let start = from
            .saturating_sub(self.dropped)
            .min(self.lines.len() as u64) as usize;
        (
            self.lines.iter().skip(start).map(String::as_str).collect(),
            missed,
        )
    }
}

/// Output as the model should see it: whole when it fits, head and tail when it
/// does not.
///
/// The interesting parts of a build log are its beginning and its end — where
/// it started and what it said last. The middle is a list of files, and paying
/// for it in tokens buys nothing.
pub fn clamp(lines: &[&str], max_bytes: usize) -> (String, bool) {
    let total: usize = lines.iter().copied().map(cost).sum();
    if total <= max_bytes {
        return (join(lines), false);
    }

    let head_budget = max_bytes / 3;
    let mut head = 0usize;
    let mut used = 0usize;
    while head < lines.len() && used + cost(lines[head]) <= head_budget {
        used += cost(lines[head]);
        head += 1;
    }

    let tail_budget = max_bytes - used;
    let mut tail = lines.len();
    used = 0;
    while tail > head && used + cost(lines[tail - 1]) <= tail_budget {
        used += cost(lines[tail - 1]);
        tail -= 1;
    }

    let skipped = tail - head;
    if skipped == 0 {
        return (join(lines), false);
    }

    let mut out = join(&lines[..head]);
    out.push_str(&format!("\n[… skipped {skipped} lines …]\n\n"));
    out.push_str(&join(&lines[tail..]));
    (out, true)
}

/// What a line costs once printed: itself and the newline after it.
fn cost(line: &str) -> usize {
    line.len() + 1
}

fn join(lines: &[&str]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_split_across_reads_survives() {
        let mut stream = Utf8Stream::default();
        let text = "café";
        let bytes = text.as_bytes();
        // Cut inside the multi-byte character, where a naive decoder puts a replacement char.
        assert_eq!(stream.push(&bytes[..4]), "caf");
        assert_eq!(stream.push(&bytes[4..]), "é");
        assert_eq!(stream.flush(), "");
    }

    #[test]
    fn bytes_that_are_not_utf8_do_not_stall_the_stream() {
        let mut stream = Utf8Stream::default();
        // What the byte in the middle turns into depends on the platform —
        // a replacement character, or whatever the console codepage says.
        let out = stream.push(&[b'a', 0xFF, b'b']);
        assert!(
            out.starts_with('a') && out.ends_with('b'),
            "got {out:?}"
        );
        assert_eq!(stream.flush(), "");
    }

    /// Older Windows tools write Russian in the OEM codepage, not UTF-8, and the feed
    /// has to show what the command printed rather than eight `�`.
    #[cfg(windows)]
    #[test]
    fn console_codepage_output_is_read_rather_than_mangled() {
        let mut stream = Utf8Stream::default();
        // Russian text as cp866 puts it on the wire.
        let bytes = [0xABu8, 0xAE, 0xAC, 0xA0, 0xAB, 0xAE, 0xE1, 0xEC];
        let out = stream.push(&bytes);

        // Which letters these are depends on the machine's codepage, so the
        // test asserts the mechanism rather than the locale: real characters
        // came back, and not the lossy reading.
        assert!(
            !out.contains(char::REPLACEMENT_CHARACTER),
            "got {out:?}"
        );
        assert_ne!(out, String::from_utf8_lossy(&bytes));
    }

    #[test]
    fn an_unfinished_tail_comes_out_on_flush() {
        let mut stream = Utf8Stream::default();
        // Half of a two-byte character, and then the pipe closed. The other
        // half is never coming, so the byte is shown as whatever the platform
        // makes of it — a replacement, or a letter of the console codepage —
        // rather than quietly dropped.
        assert_eq!(stream.push(&[0xD0]), "");
        assert_eq!(stream.flush().chars().count(), 1);
        assert_eq!(stream.flush(), "");
    }

    #[test]
    fn crlf_is_one_terminator_and_lone_cr_is_another() {
        let mut lines = Lines::default();
        assert_eq!(lines.push("a\r\nb\n"), vec!["a", "b"]);
        // A progress bar rewriting its line ends the line each time.
        assert_eq!(lines.push("10%\r20%\r"), vec!["10%", "20%"]);
        assert_eq!(lines.flush(), None);
    }

    #[test]
    fn a_line_split_across_pushes_is_still_one_line() {
        let mut lines = Lines::default();
        assert!(lines.push("hel").is_empty());
        assert_eq!(lines.push("lo\n"), vec!["hello"]);
    }

    #[test]
    fn crlf_split_across_pushes_does_not_make_an_empty_line() {
        let mut lines = Lines::default();
        assert_eq!(lines.push("a\r"), vec!["a"]);
        assert!(lines.push("\nb").is_empty());
        assert_eq!(lines.flush().as_deref(), Some("b"));
    }

    #[test]
    fn output_without_newlines_is_cut_rather_than_buffered_forever() {
        let mut lines = Lines::default();
        let long = "x".repeat(MAX_LINE_BYTES + 10);
        let out = lines.push(&long);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), MAX_LINE_BYTES);
    }

    #[test]
    fn the_ring_keeps_the_newest_and_counts_what_it_lost() {
        let mut ring = Ring::new(3);
        for i in 0..5 {
            ring.push(format!("line {i}"));
        }
        assert_eq!(ring.lines(), vec!["line 2", "line 3", "line 4"]);
        assert_eq!(ring.dropped(), 2);
        assert_eq!(ring.next_index(), 5);
    }

    #[test]
    fn reading_since_a_cursor_reports_what_was_missed() {
        let mut ring = Ring::new(3);
        for i in 0..5 {
            ring.push(format!("line {i}"));
        }

        let (lines, missed) = ring.since(3);
        assert_eq!(lines, vec!["line 3", "line 4"]);
        assert_eq!(missed, 0);

        // The cursor is older than the buffer: two lines are simply gone.
        let (lines, missed) = ring.since(0);
        assert_eq!(lines, vec!["line 2", "line 3", "line 4"]);
        assert_eq!(missed, 2);

        // Nothing new since the last read.
        let (lines, missed) = ring.since(5);
        assert!(lines.is_empty());
        assert_eq!(missed, 0);
    }

    #[test]
    fn short_output_reaches_the_model_whole() {
        let (text, cut) = clamp(&["one", "two"], 1000);
        assert_eq!(text, "one\ntwo\n");
        assert!(!cut);
    }

    #[test]
    fn long_output_keeps_the_head_and_the_tail() {
        let lines: Vec<String> = (0..1000).map(|i| format!("line {i}")).collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();

        let (text, cut) = clamp(&refs, 600);
        assert!(cut);
        assert!(text.starts_with("line 0\n"), "got: {text}");
        assert!(text.ends_with("line 999\n"), "got: {text}");
        assert!(text.contains("skipped"));
        // The budget is a promise, not a suggestion: the marker is the only
        // thing allowed past it.
        assert!(text.len() < 800, "got {} bytes", text.len());
    }
}
