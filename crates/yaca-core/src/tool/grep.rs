//! Searching file contents. A regular expression, walking while respecting `.gitignore`.

mod excerpts;
#[cfg(test)]
mod paging;

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use excerpts::{Excerpts, Hit, DEFAULT_LIMIT, MAX_LIMIT, MAX_OUTPUT_BYTES};

/// Files larger than this are almost certainly data, not code.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// How long a search may run before it is stopped.
///
/// The walk respects `.gitignore`, but a directory without it — a symlink to the
/// root of a disk, a network volume, a tree of a million files — takes the search
/// into minutes. The action line then just hangs, and the human sees a frozen app
/// rather than a working search (§0). A few minutes is enough for anything that is
/// worth waiting for.
const DEADLINE: Duration = Duration::from_secs(60);

pub struct Grep {
    /// A field so that a test can set its own: waiting a minute to check the
    /// one-minute ceiling is not a test, it is idling.
    deadline: Duration,
}

impl Grep {
    pub fn new() -> Self {
        Grep { deadline: DEADLINE }
    }
}

impl Default for Grep {
    fn default() -> Self {
        Grep::new()
    }
}

#[async_trait]
impl Tool for Grep {
    fn name(&self) -> &'static str {
        "Grep"
    }

    fn description(&self) -> &'static str {
        "Search for a regular expression in project files. Mode `content` returns \
         matching lines as `path:line: text`; with `before`, `after` or `context` it \
         adds the lines around each match as `path-line- text`, with `--` between \
         groups, which often saves reading the file. Mode `files` returns only paths. \
         A page holds 200 matches (or `head_limit`); when there are more, the last \
         line says so and gives the `offset` that continues from where it stopped. \
         By default a pattern matches inside one line; `multiline` lets it span \
         lines and makes `.` match a newline."
    }

    fn properties(&self) -> Value {
        json!({
            "pattern": {
                "type": "string",
                "description": "Regular expression in Rust regex syntax.",
            },
            "path": {
                "type": "string",
                "description": "Directory or file to search in. Defaults to every root of \
                                the session, the project first. Files in an additional \
                                root are shown by absolute path.",
            },
            "glob": {
                "type": "string",
                "description": "Restrict to a path pattern, e.g. `src/**/*.rs`, matched against \
                                the path inside the root that holds the file. A pattern with \
                                no `/`, like `*.rs`, matches file names at any depth.",
            },
            "mode": {
                "type": "string",
                "enum": ["content", "files"],
                "description": "What to return: matching lines or only paths.",
            },
            "case_insensitive": {
                "type": "boolean",
                "description": "Search case-insensitively.",
            },
            "multiline": {
                "type": "boolean",
                "description": "Match against the whole file instead of line by line, so \
                                a pattern may span lines and `.` matches a newline, like \
                                rg -U --multiline-dotall. A match is shown with every \
                                line it covers and counts once.",
            },
            "head_limit": {
                "type": "integer",
                "minimum": 1,
                "description": "Matches (or paths in `files` mode) on one page. Default 200, \
                                at most 1000.",
            },
            "offset": {
                "type": "integer",
                "minimum": 0,
                "description": "Matches (or paths) to skip, to read the next page. Take the \
                                value from the footer of the previous page.",
            },
            "context": {
                "type": "integer",
                "minimum": 0,
                "description": "Lines to show before and after each match, like grep -C. \
                                Content mode only.",
            },
            "before": {
                "type": "integer",
                "minimum": 0,
                "description": "Lines to show before each match, like grep -B. \
                                Overrides `context` on that side.",
            },
            "after": {
                "type": "integer",
                "minimum": 0,
                "description": "Lines to show after each match, like grep -A. \
                                Overrides `context` on that side.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["pattern"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Searching changes nothing.
    fn concurrent(&self) -> bool {
        true
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["pattern"].as_str().map(str::to_owned)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| ToolError::failed("pattern is required"))?;
        let multiline = input["multiline"].as_bool().unwrap_or(false);
        let re = regex::RegexBuilder::new(pattern)
            .case_insensitive(input["case_insensitive"].as_bool().unwrap_or(false))
            // `^` and `$` stay line anchors, as in ripgrep's -U, and `.` takes
            // newlines: the form a model reaching for "multiline" expects.
            .multi_line(multiline)
            .dot_matches_new_line(multiline)
            .build()
            .map_err(|e| ToolError::failed(format!("bad pattern `{pattern}`: {e}")))?;

        let given = match input["path"].as_str() {
            Some(p) => Some(ctx.ws.resolve_existing(p)?),
            None => None,
        };
        let starts = super::files::starts(&ctx.ws, given);

        let path_filter = match input["glob"].as_str() {
            Some(g) => Some(
                // As in ripgrep, `*.rs` is a file name and not a path from the
                // root: left alone it would match only files at the top level
                // and a search over a whole tree would report nothing found.
                globset::GlobBuilder::new(&if g.contains('/') {
                    g.to_string()
                } else {
                    format!("**/{g}")
                })
                    .literal_separator(true)
                    .build()
                    .map_err(|e| ToolError::failed(format!("bad glob `{g}`: {e}")))?
                    .compile_matcher(),
            ),
            None => None,
        };

        let files_only = input["mode"].as_str() == Some("files");
        let lines_of = |key: &str| input[key].as_u64().map(|n| n as usize);
        let around = lines_of("context").unwrap_or(0);
        let offset = lines_of("offset").unwrap_or(0);
        let page_size = lines_of("head_limit")
            .map(|n| n.clamp(1, MAX_LIMIT))
            .unwrap_or(DEFAULT_LIMIT);
        let mut excerpts = Excerpts::new(
            lines_of("before").unwrap_or(around),
            lines_of("after").unwrap_or(around),
            page_size,
        );
        let ws = ctx.ws.clone();
        let cancel = ctx.cancel.clone();
        let limit = self.deadline;
        let deadline = Instant::now() + limit;

        let (excerpts, files, total) = tokio::task::spawn_blocking(move || {
            let mut files: Vec<String> = Vec::new();
            let mut total = 0usize;

            // Collect what to read first, then search: the walk and the reads
            // do not get in each other's way in one hot loop.
            let mut targets: Vec<(String, std::path::PathBuf)> = Vec::new();

            for start in &starts {
                for entry in super::files::walk(start) {
                    if cancel.is_cancelled() {
                        return Err(ToolError::Cancelled);
                    }
                    if Instant::now() >= deadline {
                        return Err(overran(limit));
                    }
                    let Ok(entry) = entry else { continue };
                    if !entry.file_type().is_some_and(|t| t.is_file()) {
                        continue;
                    }
                    if entry.metadata().map(|m| m.len()).unwrap_or(0) > MAX_FILE_BYTES {
                        continue;
                    }
                    if path_filter
                        .as_ref()
                        .is_some_and(|m| !m.is_match(ws.within_root(entry.path())))
                    {
                        continue;
                    }
                    targets.push((ws.display(entry.path()), entry.path().to_path_buf()));
                }
            }

            targets.sort_by(|a, b| a.0.cmp(&b.0));
            targets.dedup_by(|a, b| a.0 == b.0);

            for (rel, path) in targets {
                if cancel.is_cancelled() {
                    return Err(ToolError::Cancelled);
                }
                if Instant::now() >= deadline {
                    return Err(overran(limit));
                }
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                if bytes.iter().take(8_192).any(|b| *b == 0) {
                    continue; // binary
                }
                let text = String::from_utf8_lossy(&bytes).into_owned();

                if files_only {
                    let found = if multiline {
                        re.is_match(&text)
                    } else {
                        text.lines().any(|line| re.is_match(line))
                    };
                    if found {
                        total += 1;
                        files.push(rel);
                    }
                    continue;
                }

                // Context needs the lines around a match, so the file is
                // split once and the matches are found by index.
                let lines: Vec<&str> = text.lines().collect();
                let hits = hits_of(&re, &text, &lines, multiline);
                if hits.is_empty() {
                    continue;
                }
                // Matches of earlier files come first, so this file's share
                // of the skipped ones is what is left of the offset.
                let skip = offset.saturating_sub(total).min(hits.len());
                total += hits.len();
                excerpts.file(&rel, &lines, &hits, skip);
                files.push(rel);
            }
            Ok((excerpts, files, total))
        })
        .await
        .map_err(|e| ToolError::failed(format!("search failed: {e}")))??;

        let mut out = String::new();
        let shown;
        if files_only {
            let mut count = 0;
            for f in files.iter().skip(offset).take(page_size) {
                out.push_str(f);
                out.push('\n');
                count += 1;
            }
            shown = count;
        } else {
            let (text, matches) = excerpts.finish();
            out = text;
            shown = matches;
        }
        let unit = if files_only { "files" } else { "matches" };
        let last = offset + shown;
        let truncated = last < total;

        if total == 0 {
            out.push_str("[no matches found]\n");
        } else if offset >= total {
            out.push_str(&format!(
                "[offset {offset} is out of range: {total} {unit} total]\n"
            ));
        } else if shown == 0 {
            // Only content mode gets here: the context asked for fills the
            // byte budget before the first match, and `offset` would not move.
            out.push_str(&format!(
                "[context exceeded {} KB before the first match; decrease \
                 context, before or after]\n",
                MAX_OUTPUT_BYTES / 1000
            ));
        } else if truncated || offset > 0 {
            let in_files = if files_only {
                String::new()
            } else {
                format!(" in {} files", files.len())
            };
            let more = if truncated {
                format!("; continue: offset={last}")
            } else {
                String::new()
            };
            out.push_str(&format!(
                "\n[showing {}–{last} of {total} {unit}{in_files}{more}]\n",
                offset + 1
            ));
        }

        let items = if files_only { files.len() } else { total };
        Ok(ToolOutput::new(out)
            .items(items as u32)
            .truncated(truncated))
    }
}

/// The lines each match covers. Line by line a match is its own line; with
/// `multiline` the pattern runs over the whole text and a match covers every
/// line from where it starts to where it ends.
fn hits_of(re: &regex::Regex, text: &str, lines: &[&str], multiline: bool) -> Vec<Hit> {
    if !multiline {
        return (0..lines.len())
            .filter(|&i| re.is_match(lines[i]))
            .map(Hit::line)
            .collect();
    }

    // Byte offsets of the newlines: a position's line is the number of them
    // before it, which is what `str::lines` counts too.
    let newlines: Vec<usize> = text.match_indices('\n').map(|(at, _)| at).collect();
    let line_of = |byte: usize| newlines.partition_point(|&at| at < byte);

    re.find_iter(text)
        .filter_map(|m| {
            let first = line_of(m.start());
            // A match that ends on a newline ends on the line before the one
            // that newline opens.
            let last = if m.end() > m.start() {
                line_of(m.end() - 1)
            } else {
                first
            };
            // An empty match after the last newline is not on any line.
            (first < lines.len()).then_some(Hit {
                first,
                last: last.min(lines.len() - 1),
            })
        })
        .collect()
}

/// The failure shape for the feed and for the model. Red (§5.9 colours `Failed`
/// `--rust`): the search was not finished, and its result is not "nothing
/// found" but "unknown".
fn overran(limit: Duration) -> ToolError {
    ToolError::failed_with(
        format!("search timed out after {} s and was stopped", limit.as_secs()),
        "Full project traversal timed out. Search in a subdirectory or narrow down `glob`.",
    )
}

#[cfg(test)]
mod tests;
