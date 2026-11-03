//! The file-facing shapes: a diff, a file's fact, the whole file for the panel,
//! and the checkpoints a rollback reads. One small edit to `src/greet.rs` runs
//! through all of them, so the Kotlin side decodes a realistic diff, not a stub.

use yaca_core::change::{
    ChangeKind, DiffLine, FileDiff, FileFact, FileView, Hunk, LineKind, Span, ViewSegment,
};
use yaca_core::checkpoint::{CheckpointChange, CheckpointSummary};

use crate::ids;
use crate::variants;

fn line(
    kind: LineKind,
    old_line: Option<u32>,
    new_line: Option<u32>,
    text: &str,
    spans: Vec<Span>,
) -> DiffLine {
    DiffLine {
        kind,
        old_line,
        new_line,
        text: text.to_owned(),
        spans,
    }
}

/// Character offsets, as the interface slices strings by characters.
fn span(start: u32, end: u32) -> Span {
    Span { start, end }
}

/// Two hunks: the greeting changes in the first, a function is added in the second.
pub fn greet_hunks() -> Vec<Hunk> {
    vec![
        Hunk {
            old_start: 1,
            old_lines: 3,
            new_start: 1,
            new_lines: 3,
            lines: vec![
                line(
                    LineKind::Context,
                    Some(1),
                    Some(1),
                    "pub fn greet() -> &'static str {",
                    vec![],
                ),
                line(
                    LineKind::Del,
                    Some(2),
                    None,
                    "    \"hello\"",
                    vec![span(4, 11)],
                ),
                line(
                    LineKind::Add,
                    None,
                    Some(2),
                    "    \"hello, world\"",
                    vec![span(4, 18)],
                ),
                line(LineKind::Context, Some(3), Some(3), "}", vec![]),
            ],
        },
        Hunk {
            old_start: 8,
            old_lines: 1,
            new_start: 8,
            new_lines: 4,
            lines: vec![
                line(
                    LineKind::Add,
                    None,
                    Some(8),
                    "pub fn farewell() -> &'static str {",
                    vec![],
                ),
                line(LineKind::Add, None, Some(9), "    \"bye\"", vec![]),
                line(LineKind::Add, None, Some(10), "}", vec![]),
                line(LineKind::Context, Some(8), Some(11), "", vec![]),
            ],
        },
    ]
}

pub fn greet_diff() -> FileDiff {
    FileDiff {
        hunks: greet_hunks(),
        added: 4,
        removed: 1,
        truncated: false,
    }
}

/// A diff the size cap cut short: no hunks survive, the counters still count
/// what was seen before the cut.
pub fn cut_diff() -> FileDiff {
    FileDiff {
        hunks: vec![],
        added: 4_000,
        removed: 12,
        truncated: true,
    }
}

/// The fact of one file action; a move carries where the file came from.
pub fn fact(path: &str, kind: ChangeKind) -> FileFact {
    let moved_from = matches!(kind, ChangeKind::Moved).then(|| "src/hello.rs".to_owned());
    FileFact {
        path: path.to_owned(),
        kind,
        moved_from,
        added: 4,
        removed: 1,
    }
}

/// The whole file as the panel shows it: unchanged text around the hunks.
pub fn view_segments() -> Vec<ViewSegment> {
    let mut hunks = greet_hunks().into_iter();
    vec![
        ViewSegment::Hunk {
            hunk: hunks.next().expect("first hunk"),
        },
        ViewSegment::Unchanged {
            lines: vec![
                line(LineKind::Context, Some(4), Some(4), "", vec![]),
                line(
                    LineKind::Context,
                    Some(5),
                    Some(5),
                    "pub fn main() {",
                    vec![],
                ),
                line(
                    LineKind::Context,
                    Some(6),
                    Some(6),
                    "    println!(\"{}\", greet());",
                    vec![],
                ),
                line(LineKind::Context, Some(7), Some(7), "}", vec![]),
            ],
        },
        ViewSegment::Hunk {
            hunk: hunks.next().expect("second hunk"),
        },
    ]
}

pub fn file_view(fact: Option<FileFact>) -> FileView {
    FileView {
        fact,
        segments: view_segments(),
        truncated: false,
    }
}

/// One checkpoint per reason, so every reason shows in the list.
pub fn checkpoint_summaries() -> Vec<CheckpointSummary> {
    variants::checkpoint_reasons()
        .into_iter()
        .enumerate()
        .map(|(i, reason)| {
            let n = i as u32 + 1;
            CheckpointSummary {
                id: ids::checkpoint(n),
                at: ids::at(60_000 * u64::from(n)),
                reason,
                tracked: 40 + n,
            }
        })
        .collect()
}

/// What a rollback would touch: one file per change kind.
pub fn checkpoint_changes() -> Vec<CheckpointChange> {
    variants::change_kinds()
        .into_iter()
        .zip([
            "src/greet.rs",
            "docs/notes.md",
            "old/legacy.txt",
            "src/hello.rs",
        ])
        .map(|(kind, path)| CheckpointChange {
            path: path.to_owned(),
            kind,
            added: 4,
            removed: 1,
        })
        .collect()
}
