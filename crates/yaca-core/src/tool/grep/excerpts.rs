//! What `Grep` hands back in content mode: matching lines, and the lines around
//! them when asked.
//!
//! The shape is ripgrep's, because it is the one models already read: `path:N:`
//! on a match, `path-N-` on a line of context, `--` between groups that do not
//! touch. Windows that overlap or touch become one group, so a line is never
//! printed twice. Without context the output is one line per match, exactly as
//! before context existed.
//!
//! A match is a span of lines, not a line: in multiline mode one match covers
//! every line it touches, and all of them are marked as the match. It counts
//! once.

/// Matches in one page unless the caller asks for another number.
pub(super) const DEFAULT_LIMIT: usize = 200;

/// The most a caller may ask for. The byte budget below still applies; this
/// only keeps a stray `head_limit: 100000` from being taken at its word.
pub(super) const MAX_LIMIT: usize = 1_000;

/// A line is cut here: a minified bundle is one line, and one match in it
/// would otherwise be the whole answer.
const MAX_LINE_CHARS: usize = 400;

/// How much one search hands the model, in bytes. The match cap alone does not
/// bound it once context is on: two hundred matches with nine lines either way
/// is well over a megabyte. The same budget as one `Read`, for the same reason.
pub(super) const MAX_OUTPUT_BYTES: usize = 100_000;

/// The lines one match covers, zero-based and inclusive. A single-line match
/// has `first == last`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Hit {
    pub first: usize,
    pub last: usize,
}

impl Hit {
    pub(super) fn line(n: usize) -> Self {
        Hit { first: n, last: n }
    }
}

pub(super) struct Excerpts {
    before: usize,
    after: usize,
    /// Matches printed at most; the rest are counted by the caller and
    /// continued from with `offset`.
    limit: usize,
    out: String,
    /// Matches printed so far.
    shown: usize,
    groups: usize,
    /// Out of room: nothing more is printed.
    full: bool,
}

impl Excerpts {
    pub(super) fn new(before: usize, after: usize, limit: usize) -> Self {
        Excerpts {
            before,
            after,
            limit,
            out: String::new(),
            shown: 0,
            groups: 0,
            full: false,
        }
    }

    /// Prints the matches of one file. `hits` are all of the file's matches,
    /// ascending and not overlapping; the first `skip` of them belong to an
    /// earlier page and are not printed. They still count as matches when they
    /// fall in the context of a later one: a line that is a match is never
    /// shown as context.
    pub(super) fn file(&mut self, rel: &str, lines: &[&str], hits: &[Hit], skip: usize) {
        let Some(last_line) = lines.len().checked_sub(1) else {
            return;
        };
        let mut i = skip;
        while i < hits.len() && !self.full {
            if self.shown >= self.limit {
                self.full = true;
                return;
            }
            let start = hits[i].first.saturating_sub(self.before);
            let mut end = hits[i].last.saturating_add(self.after).min(last_line);

            // Later matches whose window overlaps or touches this one join it,
            // as long as the group stays within the page.
            let mut j = i + 1;
            while j < hits.len()
                && self.shown + (j - i) < self.limit
                && hits[j].first.saturating_sub(self.before) <= end.saturating_add(1)
            {
                end = hits[j].last.saturating_add(self.after).min(last_line).max(end);
                j += 1;
            }
            // A group the page cut short would otherwise show the next match
            // as a line of context — a match marked as not one.
            if let Some(next) = hits.get(j) {
                end = end.min(next.first.saturating_sub(1));
            }

            self.group(rel, lines, hits, i..j, start, end);
            i = j;
        }
    }

    fn group(
        &mut self,
        rel: &str,
        lines: &[&str],
        hits: &[Hit],
        printed: std::ops::Range<usize>,
        start: usize,
        end: usize,
    ) {
        if self.groups > 0 && (self.before > 0 || self.after > 0) {
            self.out.push_str("--\n");
        }
        self.groups += 1;

        for (n, line) in lines.iter().enumerate().take(end + 1).skip(start) {
            let mark = if covered(hits, n) { ':' } else { '-' };
            let shown: String = line.chars().take(MAX_LINE_CHARS).collect();
            let row = format!("{rel}{mark}{}{mark} {}\n", n + 1, shown.trim_end());
            if self.out.len() + row.len() > MAX_OUTPUT_BYTES {
                self.full = true;
                return;
            }
            self.out.push_str(&row);
            // A match counts when its first line is printed, so one that spans
            // many lines is one match, and one cut off after its first line
            // still was shown.
            self.shown += hits[printed.clone()].iter().filter(|h| h.first == n).count();
        }
    }

    /// The text, and how many matches made it into it.
    pub(super) fn finish(self) -> (String, usize) {
        (self.out, self.shown)
    }
}

/// Whether line `n` belongs to some match. Matches are ascending and do not
/// overlap, so the only one that can cover `n` is the last to start at or
/// before it.
fn covered(hits: &[Hit], n: usize) -> bool {
    let at = hits.partition_point(|h| h.first <= n);
    at > 0 && hits[at - 1].last >= n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("l{i}")).collect()
    }

    fn singles(at: &[usize]) -> Vec<Hit> {
        at.iter().map(|&n| Hit::line(n)).collect()
    }

    fn page(before: usize, after: usize, limit: usize, n: usize, hits: &[Hit], skip: usize) -> (String, usize) {
        let owned = lines(n);
        let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
        let mut e = Excerpts::new(before, after, limit);
        e.file("f", &refs, hits, skip);
        e.finish()
    }

    fn run(before: usize, after: usize, n: usize, at: &[usize]) -> (String, usize) {
        page(before, after, DEFAULT_LIMIT, n, &singles(at), 0)
    }

    #[test]
    fn windows_that_only_touch_still_merge() {
        // Match on 1 shows 1–2, match on 3 shows 3–4: adjacent, so no `--`.
        let (text, shown) = run(0, 1, 5, &[0, 2]);
        assert_eq!(text, "f:1: l1\nf-2- l2\nf:3: l3\nf-4- l4\n");
        assert_eq!(shown, 2);
        // One line apart they are two groups.
        let (text, _) = run(0, 1, 5, &[0, 3]);
        assert_eq!(text, "f:1: l1\nf-2- l2\n--\nf:4: l4\nf-5- l5\n");
    }

    #[test]
    fn context_stops_at_the_ends_of_the_file() {
        let (text, _) = run(5, 5, 3, &[1]);
        assert_eq!(text, "f-1- l1\nf:2: l2\nf-3- l3\n");
    }

    #[test]
    fn a_huge_request_for_context_does_not_overflow() {
        let (text, shown) = run(usize::MAX, usize::MAX, 3, &[1]);
        assert_eq!(text, "f-1- l1\nf:2: l2\nf-3- l3\n");
        assert_eq!(shown, 1);
    }

    #[test]
    fn the_page_limit_holds_inside_one_merged_group() {
        // Every line matches, so every window touches the next: one group that
        // would otherwise print them all.
        let all: Vec<usize> = (0..DEFAULT_LIMIT + 50).collect();
        let (text, shown) = run(1, 1, DEFAULT_LIMIT + 50, &all);
        assert_eq!(shown, DEFAULT_LIMIT);
        assert_eq!(
            text.lines().filter(|l| l.starts_with("f:")).count(),
            DEFAULT_LIMIT
        );
        // The first match left out is not smuggled in as context.
        assert!(
            !text.contains(&format!("f-{}-", DEFAULT_LIMIT + 1)),
            "{}",
            text.lines().last().unwrap()
        );
    }

    #[test]
    fn a_page_starts_after_the_matches_that_were_skipped() {
        let (text, shown) = page(0, 0, 2, 10, &singles(&[0, 2, 4, 6, 8]), 2);
        assert_eq!(text, "f:5: l5\nf:7: l7\n");
        assert_eq!(shown, 2);
    }

    #[test]
    fn a_skipped_match_is_not_shown_as_context_of_the_next() {
        // Match 0 was on the previous page; match 1 is one line below it. Its
        // context reaches line 0, which is a match and keeps its mark.
        let (text, shown) = page(1, 0, 5, 4, &singles(&[0, 1]), 1);
        assert_eq!(text, "f:1: l1\nf:2: l2\n");
        assert_eq!(shown, 1);
    }

    #[test]
    fn a_match_over_several_lines_is_marked_throughout_and_counted_once() {
        let hits = [Hit { first: 1, last: 3 }];
        let (text, shown) = page(1, 1, DEFAULT_LIMIT, 6, &hits, 0);
        assert_eq!(text, "f-1- l1\nf:2: l2\nf:3: l3\nf:4: l4\nf-5- l5\n");
        assert_eq!(shown, 1);
    }

    #[test]
    fn two_multiline_matches_do_not_swallow_the_lines_between_them() {
        let hits = [Hit { first: 0, last: 1 }, Hit { first: 4, last: 5 }];
        let (text, shown) = page(0, 0, DEFAULT_LIMIT, 7, &hits, 0);
        assert_eq!(text, "f:1: l1\nf:2: l2\nf:5: l5\nf:6: l6\n");
        assert_eq!(shown, 2);
    }
}
