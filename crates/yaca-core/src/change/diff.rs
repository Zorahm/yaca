//! Diff calculation (§6.3).
//!
//! Computed in the core, not in the webview, for three reasons: the outcome is identical in
//! the feed and the panel, it lands in the event stream as data, and §12 requires
//! 60 fps during streaming — parsing text in webview concurrently with deltas
//! would not work.

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, InlineChange, TextDiff};
use ts_rs::TS;

/// Context lines around the change. Three is what everyone shows,
/// and enough to recognize the location.
const CONTEXT: usize = 3;

/// §12: a diff longer than two thousand lines is rendered windowed. Past this
/// limit the core has nothing to give — the UI will show only part of it anyway.
const MAX_LINES: usize = 2_000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub hunks: Vec<Hunk>,
    pub added: u32,
    pub removed: u32,
    /// Diff is cut off at [`MAX_LINES`].
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

impl Hunk {
    /// `@@ -12,7 +12,9 @@` — hunk header (§6.3).
    pub fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old_start, self.old_lines, self.new_start, self.new_lines
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    /// Line number before edit, 1-based. Left gutter (§6.3).
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
    /// What exactly changed inside the line — word-level highlighting (§6.3).
    /// Spans in characters, not in bytes: the interface slices strings by characters.
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    Context,
    Add,
    Del,
}

pub fn compute(before: &str, after: &str) -> FileDiff {
    let diff = TextDiff::from_lines(before, after);

    let mut out = FileDiff::default();

    // Counts are computed across all operations, not just rendered lines:
    // `+597 −16` in the bar (§6.1) is about the whole file, even if a slice is shown.
    for op in diff.ops() {
        use similar::DiffTag;
        match op.tag() {
            DiffTag::Delete => out.removed += op.old_range().len() as u32,
            DiffTag::Insert => out.added += op.new_range().len() as u32,
            DiffTag::Replace => {
                out.removed += op.old_range().len() as u32;
                out.added += op.new_range().len() as u32;
            }
            DiffTag::Equal => {}
        }
    }

    let mut emitted = 0usize;

    'groups: for group in diff.grouped_ops(CONTEXT) {
        let Some(first) = group.first() else { continue };
        let old_start = first.old_range().start as u32 + 1;
        let new_start = first.new_range().start as u32 + 1;

        let mut lines: Vec<DiffLine> = Vec::new();
        let mut truncated = false;

        for op in &group {
            for change in diff.iter_inline_changes(op) {
                // Limit is checked on every line, not on every hunk.
                // Otherwise one hunk of 5000 lines blows past the limit:
                // contiguous changes are grouped into one hunk.
                if emitted >= MAX_LINES {
                    truncated = true;
                    break;
                }

                let kind = match change.tag() {
                    ChangeTag::Equal => LineKind::Context,
                    ChangeTag::Delete => LineKind::Del,
                    ChangeTag::Insert => LineKind::Add,
                };
                let (text, spans) = inline(&change);
                lines.push(DiffLine {
                    kind,
                    old_line: change.old_index().map(|i| i as u32 + 1),
                    new_line: change.new_index().map(|i| i as u32 + 1),
                    text,
                    spans,
                });
                emitted += 1;
            }
            if truncated {
                break;
            }
        }

        // Lengths are computed over emitted lines: the hunk header must
        // describe what is in it, not what could have been in it.
        push_hunk(&mut out, old_start, new_start, lines);

        if truncated {
            out.truncated = true;
            break 'groups;
        }
    }

    out
}

fn push_hunk(out: &mut FileDiff, old_start: u32, new_start: u32, lines: Vec<DiffLine>) {
    if lines.is_empty() {
        return;
    }
    let old_lines = lines.iter().filter(|l| l.old_line.is_some()).count() as u32;
    let new_lines = lines.iter().filter(|l| l.new_line.is_some()).count() as u32;
    out.hunks.push(Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines,
    });
}

/// Collects line text and spans of changed chunks.
pub(super) fn inline(change: &InlineChange<'_, str>) -> (String, Vec<Span>) {
    let mut text = String::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut chars = 0u32;

    for (emphasized, value) in change.iter_strings_lossy() {
        let start = chars;
        text.push_str(&value);
        chars += value.chars().count() as u32;
        if emphasized && chars > start {
            // Merge adjacent highlighted spans: it is simpler for the UI
            // to render one range than a chain of three letters.
            match spans.last_mut() {
                Some(prev) if prev.end == start => prev.end = chars,
                _ => spans.push(Span { start, end: chars }),
            }
        }
    }

    // Newline does not go into diff: gutter and background are drawn for the line as a whole.
    let trimmed = text.trim_end_matches(['\n', '\r']);
    let limit = trimmed.chars().count() as u32;
    text.truncate(trimmed.len());

    spans.retain(|s| s.start < limit);
    for s in &mut spans {
        s.end = s.end.min(limit);
    }

    (text, spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_added_and_removed() {
        let d = compute("one\ntwo\nthree\n", "one\ntwo and a half\nthree\nfour\n");
        assert_eq!((d.added, d.removed), (2, 1));
        assert!(!d.truncated);
    }

    #[test]
    fn identical_files_produce_empty_diff() {
        let d = compute("same\n", "same\n");
        assert!(d.hunks.is_empty());
        assert_eq!((d.added, d.removed), (0, 0));
    }

    #[test]
    fn line_numbers_start_at_one() {
        let d = compute("a\nb\nc\n", "a\nB\nc\n");
        let lines = &d.hunks[0].lines;

        let del = lines.iter().find(|l| l.kind == LineKind::Del).unwrap();
        assert_eq!(del.old_line, Some(2));
        assert_eq!(del.new_line, None);
        assert_eq!(del.text, "b");

        let add = lines.iter().find(|l| l.kind == LineKind::Add).unwrap();
        assert_eq!(add.new_line, Some(2));
        assert_eq!(add.old_line, None);

        let ctx = lines.iter().find(|l| l.kind == LineKind::Context).unwrap();
        assert_eq!((ctx.old_line, ctx.new_line), (Some(1), Some(1)));
    }

    #[test]
    fn newline_does_not_go_into_text() {
        let d = compute("a\n", "b\n");
        for line in &d.hunks[0].lines {
            assert!(
                !line.text.contains('\n'),
                "newline in line text: {line:?}"
            );
        }
    }

    #[test]
    fn word_level_highlighting_marks_only_changed_parts() {
        let d = compute("let x = 1;\n", "let x = 42;\n");
        let add = d.hunks[0]
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Add)
            .unwrap();

        assert!(!add.spans.is_empty(), "highlighting must be present");
        // Spans must lie inside the line, otherwise the interface slices it
        // in the wrong place and shows garbage.
        let len = add.text.chars().count() as u32;
        for s in &add.spans {
            assert!(s.start < s.end && s.end <= len, "{s:?} outside line {len}");
        }

        let highlighted: String = add
            .spans
            .iter()
            .flat_map(|s| {
                add.text
                    .chars()
                    .skip(s.start as usize)
                    .take((s.end - s.start) as usize)
            })
            .collect();
        assert!(highlighted.contains("42"), "highlighted: {highlighted:?}");
        assert!(!highlighted.contains("let"), "no context to highlight");
    }

    #[test]
    fn multi_byte_characters_do_not_break_spans() {
        let d = compute("café world\n", "café city\n");
        let add = d.hunks[0]
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Add)
            .unwrap();
        let len = add.text.chars().count() as u32;
        for s in &add.spans {
            assert!(s.end <= len, "{s:?} outside line length {len}");
        }
    }

    #[test]
    fn file_creation_is_one_hunk_of_additions() {
        let d = compute("", "first\nsecond\n");
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(d.added, 2);
        assert_eq!(d.removed, 0);
        assert!(d.hunks[0].lines.iter().all(|l| l.kind == LineKind::Add));
    }

    #[test]
    fn huge_diff_is_truncated_rather_than_dumped_whole() {
        let before: String = (0..5_000).map(|i| format!("line {i}\n")).collect();
        let after: String = (0..5_000).map(|i| format!("LINE {i}\n")).collect();
        let d = compute(&before, &after);
        assert!(d.truncated);
        let total: usize = d.hunks.iter().map(|h| h.lines.len()).sum();
        assert_eq!(total, MAX_LINES, "emitted {total} lines");
        // Bar counts are for the whole file, not the shown chunk.
        assert_eq!((d.added, d.removed), (5_000, 5_000));
    }

    #[test]
    fn hunk_header_reads_like_a_diff() {
        let d = compute("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!(d.hunks[0].header(), "@@ -1,3 +1,3 @@");
    }
}
