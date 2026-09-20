//! What `Grep` hands back in content mode: matching lines, and the lines around
//! them when asked.
//!
//! The shape is ripgrep's, because it is the one models already read: `path:N:`
//! on a match, `path-N-` on a line of context, `--` between groups that do not
//! touch. Windows that overlap or touch become one group, so a line is never
//! printed twice. Without context the output is one line per match, exactly as
//! before context existed.

/// Matches shown at most. Past this the footer says how many there were.
pub(super) const MAX_MATCHES: usize = 200;

/// A line is cut here: a minified bundle is one line, and one match in it
/// would otherwise be the whole answer.
const MAX_LINE_CHARS: usize = 400;

/// How much one search hands the model, in bytes. The match cap alone does not
/// bound it once context is on: two hundred matches with nine lines either way
/// is well over a megabyte. The same budget as one `Read`, for the same reason.
const MAX_OUTPUT_BYTES: usize = 100_000;

pub(super) struct Excerpts {
    before: usize,
    after: usize,
    out: String,
    /// Matches printed so far.
    shown: usize,
    groups: usize,
    /// Out of room: nothing more is printed, matches are still counted.
    full: bool,
}

impl Excerpts {
    pub(super) fn new(before: usize, after: usize) -> Self {
        Excerpts {
            before,
            after,
            out: String::new(),
            shown: 0,
            groups: 0,
            full: false,
        }
    }

    /// Prints the matches of one file. `hits` are line indices, ascending.
    pub(super) fn file(&mut self, rel: &str, lines: &[&str], hits: &[usize]) {
        let Some(last) = lines.len().checked_sub(1) else {
            return;
        };
        let mut i = 0;
        while i < hits.len() && !self.full {
            if self.shown >= MAX_MATCHES {
                self.full = true;
                return;
            }
            let start = hits[i].saturating_sub(self.before);
            let mut end = hits[i].saturating_add(self.after).min(last);

            // Later matches whose window overlaps or touches this one join it,
            // as long as the group stays within the match cap.
            let mut j = i + 1;
            while j < hits.len()
                && self.shown + (j - i) < MAX_MATCHES
                && hits[j].saturating_sub(self.before) <= end.saturating_add(1)
            {
                end = hits[j].saturating_add(self.after).min(last).max(end);
                j += 1;
            }
            // A group the cap cut short would otherwise show the next match
            // as a line of context — a match marked as not one.
            if let Some(next) = hits.get(j) {
                end = end.min(next - 1);
            }

            self.group(rel, lines, &hits[i..j], start, end);
            i = j;
        }
    }

    fn group(&mut self, rel: &str, lines: &[&str], hits: &[usize], start: usize, end: usize) {
        if self.groups > 0 && (self.before > 0 || self.after > 0) {
            self.out.push_str("--\n");
        }
        self.groups += 1;

        for (n, line) in lines.iter().enumerate().take(end + 1).skip(start) {
            let hit = hits.binary_search(&n).is_ok();
            let mark = if hit { ':' } else { '-' };
            let shown: String = line.chars().take(MAX_LINE_CHARS).collect();
            let row = format!("{rel}{mark}{}{mark} {}\n", n + 1, shown.trim_end());
            if self.out.len() + row.len() > MAX_OUTPUT_BYTES {
                self.full = true;
                return;
            }
            self.out.push_str(&row);
            if hit {
                self.shown += 1;
            }
        }
    }

    /// The text, and how many matches made it into it.
    pub(super) fn finish(self) -> (String, usize) {
        (self.out, self.shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(n: usize) -> Vec<String> {
        (1..=n).map(|i| format!("l{i}")).collect()
    }

    fn run(before: usize, after: usize, n: usize, hits: &[usize]) -> (String, usize) {
        let owned = lines(n);
        let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
        let mut e = Excerpts::new(before, after);
        e.file("f", &refs, hits);
        e.finish()
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
    fn the_match_cap_holds_inside_one_merged_group() {
        // Every line matches, so every window touches the next: one group that
        // would otherwise print them all.
        let hits: Vec<usize> = (0..MAX_MATCHES + 50).collect();
        let (text, shown) = run(1, 1, MAX_MATCHES + 50, &hits);
        assert_eq!(shown, MAX_MATCHES);
        assert_eq!(text.lines().filter(|l| l.starts_with("f:")).count(), MAX_MATCHES);
        // The first match left out is not smuggled in as context.
        assert!(!text.contains(&format!("f-{}-", MAX_MATCHES + 1)), "{}", text.lines().last().unwrap());
    }
}
