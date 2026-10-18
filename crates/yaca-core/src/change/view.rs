//! The whole file as the panel shows it (§6.4): every line, and the changed
//! regions as hunks.
//!
//! A hunk here is a maximal run of changed lines with no unchanged line between
//! them. It is deliberately not the three-line-context group of the feed: the
//! feed shows what one call did, the panel shows everything the session changed
//! in the file.

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use ts_rs::TS;

use super::diff::{inline, DiffLine, Hunk, LineKind};
use super::FileFact;

/// The most lines a view ships. A file past this is cut at a hunk boundary and
/// marked truncated: an honest cut beats a stream that stalls the panel (§12).
pub const MAX_VIEW_LINES: usize = 20_000;

/// One piece of the file, in order. Unchanged text is never split into hunks;
/// the interface decides how much of it to fold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ViewSegment {
    Unchanged { lines: Vec<DiffLine> },
    Hunk { hunk: Hunk },
}

/// A file's full diff against its baseline, with the unchanged lines between
/// the hunks (§6.4). A file the session has not changed is the same shape with
/// no hunks in it: the panel shows a file the agent only read as well.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    /// `None` — the session has not changed the file, so there is nothing to
    /// say about a change.
    pub fact: Option<FileFact>,
    pub segments: Vec<ViewSegment>,
    /// The view stops before the rest of the file. Hunks past the cut are not
    /// shown, so they cannot be decided from the panel either.
    pub truncated: bool,
}

/// The hunks between two texts, with their lines, in file order. Changed lines
/// only: the unchanged ones are not part of any hunk.
pub fn hunks(before: &str, after: &str) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(before, after);

    diff.grouped_ops(0)
        .iter()
        .filter_map(|group| {
            let first = group.first()?;
            let last = group.last()?;
            let old = first.old_range().start..last.old_range().end;
            let new = first.new_range().start..last.new_range().end;

            let mut lines = Vec::new();
            for op in group {
                for change in diff.iter_inline_changes(op) {
                    let kind = match change.tag() {
                        ChangeTag::Delete => LineKind::Del,
                        ChangeTag::Insert => LineKind::Add,
                        // A group with no context holds no equal line; if one
                        // ever showed up it would be drawn as context, not lost.
                        ChangeTag::Equal => LineKind::Context,
                    };
                    let (text, spans) = inline(&change);
                    lines.push(DiffLine {
                        kind,
                        old_line: change.old_index().map(|i| i as u32 + 1),
                        new_line: change.new_index().map(|i| i as u32 + 1),
                        text,
                        spans,
                    });
                }
            }

            Some(Hunk {
                // Starts are 1-based like the feed's. An insertion has an empty
                // old range; its start is the line it goes in front of, which
                // keeps the identity unique within the file.
                old_start: old.start as u32 + 1,
                old_lines: old.len() as u32,
                new_start: new.start as u32 + 1,
                new_lines: new.len() as u32,
                lines,
            })
        })
        .collect()
}

/// The full view of one file: hunks interleaved with the unchanged text between
/// them. `after` is what is on disk now, so unchanged lines come from it.
pub fn view(before: &str, after: &str, fact: Option<FileFact>) -> FileView {
    let before_lines: Vec<&str> = before.split_inclusive('\n').collect();
    let all = hunks(before, after);

    let mut segments = Vec::new();
    let mut emitted = 0usize;
    let mut truncated = false;
    // Positions in the old and new text of the first line not yet shown.
    let mut old_at = 0usize;
    let mut new_at = 0usize;

    for hunk in &all {
        let start = hunk.old_start as usize - 1;
        let gap = start - old_at;
        let hunk_len = hunk.lines.len();

        // The gap and the hunk are shown together or not at all: a hunk cut in
        // the middle would be a diff that lies about what the file contains.
        if emitted + gap + hunk_len > MAX_VIEW_LINES {
            truncated = true;
            break;
        }

        if gap > 0 {
            let lines = unchanged(&before_lines, old_at..start, new_at);
            emitted += lines.len();
            segments.push(ViewSegment::Unchanged { lines });
        }
        emitted += hunk_len;
        segments.push(ViewSegment::Hunk { hunk: hunk.clone() });

        old_at = start + hunk.old_lines as usize;
        new_at = hunk.new_start as usize - 1 + hunk.new_lines as usize;
    }

    if !truncated && old_at < before_lines.len() {
        // The tail after the last hunk. Its length is whatever is left of the
        // budget; a file longer than the budget is truncated here.
        let tail = before_lines.len() - old_at;
        if emitted + tail > MAX_VIEW_LINES {
            truncated = true;
        } else {
            segments.push(ViewSegment::Unchanged {
                lines: unchanged(&before_lines, old_at..before_lines.len(), new_at),
            });
        }
    }

    FileView {
        fact,
        segments,
        truncated,
    }
}

/// Lines equal in both versions, numbered on both sides. `new_at` is where the
/// first of them sits in the new text.
fn unchanged(before_lines: &[&str], old: std::ops::Range<usize>, new_at: usize) -> Vec<DiffLine> {
    old.enumerate()
        .map(|(k, i)| DiffLine {
            kind: LineKind::Context,
            old_line: Some(i as u32 + 1),
            new_line: Some((new_at + k) as u32 + 1),
            text: before_lines[i]
                .trim_end_matches(['\n', '\r'])
                .to_owned(),
            spans: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::ChangeKind;

    fn fact() -> FileFact {
        FileFact {
            path: "a.txt".into(),
            kind: ChangeKind::Modified,
            moved_from: None,
            added: 0,
            removed: 0,
        }
    }

    /// Rebuilds the after-text from a view: unchanged and added lines in order.
    /// If the view lost or invented a line, this does not come out equal.
    fn rebuild(view: &FileView) -> String {
        let mut out = String::new();
        for seg in &view.segments {
            let lines: &[DiffLine] = match seg {
                ViewSegment::Unchanged { lines } => lines,
                ViewSegment::Hunk { hunk } => &hunk.lines,
            };
            for line in lines {
                if line.kind != LineKind::Del {
                    out.push_str(&line.text);
                    out.push('\n');
                }
            }
        }
        out
    }

    #[test]
    fn two_far_apart_changes_are_two_hunks() {
        let before: String = (1..=30).map(|i| format!("line {i}\n")).collect();
        let after = before
            .replace("line 2\n", "edit 2\n")
            .replace("line 25\n", "edit 25\n");
        let hunks = hunks(&before, &after);
        assert_eq!(hunks.len(), 2, "{hunks:?}");
        assert_eq!(hunks[0].old_start, 2);
        assert_eq!(hunks[1].old_start, 25);
    }

    #[test]
    fn view_keeps_every_line_of_the_after_text() {
        let before = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let after = "a\nB\nc\nd\ne\nf\nG\nh\nnew\n";
        let v = view(before, after, Some(fact()));
        assert!(!v.truncated);
        assert_eq!(rebuild(&v), after);
    }

    #[test]
    fn view_of_an_untouched_file_is_one_unchanged_segment() {
        let v = view("x\ny\n", "x\ny\n", None);
        assert_eq!(v.segments.len(), 1);
        assert!(matches!(v.segments[0], ViewSegment::Unchanged { .. }));
    }

    #[test]
    fn view_is_cut_at_a_hunk_boundary_when_too_long() {
        let before: String = (0..MAX_VIEW_LINES + 10).map(|i| format!("{i}\n")).collect();
        let after = format!("CHANGED\n{}", before.split_once('\n').unwrap().1);
        let v = view(&before, &after, Some(fact()));
        assert!(v.truncated);
        let shown: usize = v
            .segments
            .iter()
            .map(|s| match s {
                ViewSegment::Unchanged { lines } => lines.len(),
                ViewSegment::Hunk { hunk } => hunk.lines.len(),
            })
            .sum();
        assert!(shown <= MAX_VIEW_LINES);
    }
}
