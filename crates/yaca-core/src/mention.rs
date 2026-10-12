//! The `@` file mention search (§7.2): the answer behind the menu.
//!
//! The walk is the one `grep` and `glob` take ([`crate::tool::files::walk`]),
//! so a file the agent cannot reach through those tools is not offered here,
//! and ignore rules are honoured the same way. Matching is a case-insensitive
//! subsequence test on the project-relative path with forward slashes. Ranking
//! goes by match quality first, then by files this session has read, then by
//! the newer modification time: a file the human is looking for by name beats a
//! file the agent happened to open last week, and between two equal matches the
//! one the agent has read is the more likely target.

use std::time::UNIX_EPOCH;

use crate::workspace::Workspace;

/// How many answers one search returns. The menu shows about this many rows;
/// a longer list would only be scrolled past.
pub const LIMIT: usize = 20;

/// One file that matches the query, with what ranking needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hit {
    pub(crate) path: String,
    pub(crate) score: i64,
    /// Read in this session (`Workspace::was_read`).
    pub(crate) read: bool,
    /// Milliseconds since the epoch; 0 when the time cannot be read.
    pub(crate) mtime: u64,
}

/// The project's files matching `query`, best first, at most [`LIMIT`].
///
/// Blocking: it walks the tree. The session runs it on a blocking task.
///
/// An empty query matches everything, and the answer is then the files this
/// session read and the files changed last. That is what an empty `@` should
/// offer: the files most likely to be the one meant.
pub fn search(ws: &Workspace, query: &str) -> Vec<String> {
    let mut hits = Vec::new();
    // Every root, the project first, as `grep` without a path walks them. An
    // additional root's files are shown by absolute path (`Workspace::display`).
    for start in crate::tool::files::starts(ws, None) {
        for entry in crate::tool::files::walk(&start) {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let rel = ws.display(path);
            let Some(score) = score(query, &rel) else {
                continue;
            };
            hits.push(Hit {
                score,
                read: ws.was_read(path),
                mtime: mtime_ms(&entry),
                path: rel,
            });
        }
    }
    rank(hits)
        .into_iter()
        .take(LIMIT)
        .map(|hit| hit.path)
        .collect()
}

fn mtime_ms(entry: &ignore::DirEntry) -> u64 {
    entry
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// How well `query` matches `path` as a subsequence, or `None` when it does not.
///
/// Both sides are lower-cased and the query's whitespace is ignored. Each
/// matched character scores. A character that starts a word (at the start of
/// the path, or after `/`, `.`, `_`, `-`) scores more, and so does one right
/// after the previous match. Every character skipped between matches costs a
/// little, so a query spread thinly over the path ranks below one that sits
/// together. A query that lies wholly inside the file name gets a bonus, and
/// among equal matches a shorter path goes first.
///
/// An empty query scores 0 for every path: nothing is being matched, so
/// nothing separates one path from another here.
pub(crate) fn score(query: &str, path: &str) -> Option<i64> {
    let needle: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if needle.is_empty() {
        return Some(0);
    }
    let hay: Vec<char> = path.chars().flat_map(char::to_lowercase).collect();

    let mut score = 0i64;
    let mut from = 0usize;
    let mut previous: Option<usize> = None;
    for &want in &needle {
        let found = (from..hay.len()).find(|&i| hay[i] == want)?;
        score += 16;
        if found == 0 || matches!(hay[found - 1], '/' | '.' | '_' | '-') {
            score += 12;
        }
        match previous {
            Some(prev) if found == prev + 1 => score += 8,
            Some(prev) => score -= ((found - prev - 1) as i64).min(5),
            None => score -= (found as i64).min(5),
        }
        previous = Some(found);
        from = found + 1;
    }

    let name_start = hay.iter().rposition(|&c| c == '/').map_or(0, |i| i + 1);
    if hay[name_start..]
        .windows(needle.len())
        .any(|window| window == needle.as_slice())
    {
        score += 40;
    }

    score -= hay.len() as i64 / 8;
    Some(score)
}

/// Orders the hits: score, then read in this session, then newer, then by path
/// so that equal files come out the same way on every run.
pub(crate) fn rank(mut hits: Vec<Hit>) -> Vec<Hit> {
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(b.read.cmp(&a.read))
            .then(b.mtime.cmp(&a.mtime))
            .then(a.path.cmp(&b.path))
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(path: &str, score: i64, read: bool, mtime: u64) -> Hit {
        Hit {
            path: path.into(),
            score,
            read,
            mtime,
        }
    }

    fn paths(hits: Vec<Hit>) -> Vec<String> {
        hits.into_iter().map(|h| h.path).collect()
    }

    #[test]
    fn a_subsequence_matches_and_anything_else_does_not() {
        assert!(score("sra", "src/app.rs").is_some());
        assert!(score("ra", "src/app.rs").is_some());
        assert!(score("zz", "src/app.rs").is_none());
        // Order matters: `sar` is there in order, `asr` has no `r` after its `s`.
        assert!(score("sar", "src/app.rs").is_some());
        assert!(score("asr", "src/app.rs").is_none());
        assert!(score("rsa", "src/app.rs").is_none());
    }

    #[test]
    fn matching_ignores_case_and_whitespace() {
        assert!(score("APP", "src/app.rs").is_some());
        assert!(score("app", "src/App.tsx").is_some());
        assert!(score("a p", "src/app.rs").is_some());
    }

    #[test]
    fn an_empty_query_scores_every_path_alike() {
        assert_eq!(score("", "a.rs"), score("", "a/very/long/path/to/file.rs"));
        assert_eq!(score("  ", "a.rs"), Some(0));
    }

    #[test]
    fn a_match_at_word_starts_beats_the_same_letters_inside_a_word() {
        let at_word = score("app", "src/app.rs").unwrap();
        let inside = score("app", "src/zapzzzzp.rs").unwrap();
        assert!(at_word > inside, "{at_word} vs {inside}");
    }

    #[test]
    fn a_query_inside_the_file_name_beats_the_same_letters_in_directories() {
        let name = score("main", "src/main.rs").unwrap();
        let dirs = score("main", "m/a/i/n/x.rs").unwrap();
        assert!(name > dirs, "{name} vs {dirs}");
    }

    #[test]
    fn match_quality_comes_before_being_read_and_before_age() {
        let ranked = rank(vec![
            hit("weak.rs", 10, true, 900),
            hit("strong.rs", 50, false, 1),
        ]);
        assert_eq!(paths(ranked), vec!["strong.rs", "weak.rs"]);
    }

    #[test]
    fn among_equal_matches_the_read_file_comes_first_then_the_newer_one() {
        let ranked = rank(vec![
            hit("old.rs", 5, false, 100),
            hit("new.rs", 5, false, 200),
            hit("read.rs", 5, true, 1),
        ]);
        assert_eq!(paths(ranked), vec!["read.rs", "new.rs", "old.rs"]);
    }

    #[test]
    fn ties_are_broken_by_path_so_the_order_is_the_same_each_run() {
        let ranked = rank(vec![hit("b.rs", 5, false, 7), hit("a.rs", 5, false, 7)]);
        assert_eq!(paths(ranked), vec!["a.rs", "b.rs"]);
    }
}
