//! What a language server answers, as text for a model — and what a model
//! says, as a position a server understands.
//!
//! LSP positions are zero-based lines and UTF-16 code units; a model counts
//! from one, in characters, and is bad at counting columns at all. So the
//! model names a line and the text to stand on, and the column is found here.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::uri::path_of;

/// A source line longer than this is cut when shown next to a location.
const SNIPPET_CHARS: usize = 160;

/// A file bigger than this is not read for snippets.
const SNIPPET_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// A zero-based position as a server states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Place {
    pub path: PathBuf,
    pub line: u32,
    /// UTF-16 code units from the start of the line.
    pub character: u32,
}

/// Where in `text` to ask. `line` counts from one.
///
/// `symbol` is text on that line to stand at the start of; without it, and
/// without `column`, the first character that is not blank. An ambiguous
/// `symbol` is an error that says so, because guessing which of two `x`
/// was meant would answer a question nobody asked.
pub(crate) fn locate(
    text: &str,
    line: usize,
    symbol: Option<&str>,
    column: Option<usize>,
) -> Result<(u32, u32), String> {
    let lines: Vec<&str> = text.lines().collect();
    if line == 0 || line > lines.len() {
        return Err(format!("в файле {} строк, запрошена строка {line}", lines.len()));
    }
    let content = lines[line - 1];

    let chars = match (symbol, column) {
        (_, Some(column)) if column >= 1 => column - 1,
        (Some(symbol), _) if !symbol.is_empty() => stand_on(content, symbol, line)?,
        _ => content.chars().take_while(|c| c.is_whitespace()).count(),
    };
    Ok(((line - 1) as u32, utf16_col(content, chars)))
}

fn stand_on(content: &str, symbol: &str, line: usize) -> Result<usize, String> {
    let hits: Vec<usize> = content.match_indices(symbol).map(|(byte, _)| byte).collect();
    if hits.is_empty() {
        return Err(format!("в строке {line} нет «{symbol}»: {}", content.trim()));
    }
    // A whole word is what is meant when the text is a name: `id` in
    // `valid(id)`, not the end of `valid`.
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let whole: Vec<usize> = hits
        .iter()
        .copied()
        .filter(|&at| {
            let before = content[..at].chars().next_back();
            let after = content[at + symbol.len()..].chars().next();
            !before.is_some_and(word) && !after.is_some_and(word)
        })
        .collect();
    let candidates = if whole.is_empty() { &hits } else { &whole };
    if candidates.len() > 1 {
        let columns: Vec<String> = candidates
            .iter()
            .map(|&at| (content[..at].chars().count() + 1).to_string())
            .collect();
        return Err(format!(
            "«{symbol}» встречается в строке {line} несколько раз (колонки {}): \
             возьми более длинный фрагмент или задай column",
            columns.join(", ")
        ));
    }
    Ok(content[..candidates[0]].chars().count())
}

fn utf16_col(content: &str, chars: usize) -> u32 {
    content.chars().take(chars).map(|c| c.len_utf16() as u32).sum()
}

/// The inverse, for showing a server's column to a human.
fn chars_of(content: &str, units: u32) -> u32 {
    let mut seen = 0;
    let mut chars = 0;
    for c in content.chars() {
        if seen >= units {
            break;
        }
        seen += c.len_utf16() as u32;
        chars += 1;
    }
    chars
}

/// Source lines read once per file.
#[derive(Default)]
pub(crate) struct Sources(HashMap<PathBuf, Option<Vec<String>>>);

impl Sources {
    fn lines(&mut self, path: &Path) -> Option<&Vec<String>> {
        self.0
            .entry(path.to_path_buf())
            .or_insert_with(|| {
                let size = std::fs::metadata(path).ok()?.len();
                if size > SNIPPET_FILE_BYTES {
                    return None;
                }
                let text = String::from_utf8_lossy(&std::fs::read(path).ok()?).into_owned();
                Some(text.lines().map(str::to_string).collect())
            })
            .as_ref()
    }

    /// `(one-based column in characters, the line trimmed and cut)`.
    fn at(&mut self, place: &Place) -> (u32, Option<String>) {
        let Some(line) = self
            .lines(&place.path)
            .and_then(|lines| lines.get(place.line as usize))
        else {
            return (place.character + 1, None);
        };
        let column = chars_of(line, place.character) + 1;
        let shown: String = line.trim().chars().take(SNIPPET_CHARS).collect();
        (column, Some(shown))
    }
}

/// The places in an answer to definition, references and the like: one
/// `Location`, a list of them, or a list of `LocationLink`.
pub(crate) fn places(answer: &Value) -> Vec<Place> {
    let one = |value: &Value| -> Option<Place> {
        let (uri, range) = match value.get("targetUri") {
            Some(uri) => (
                uri.as_str()?,
                value.get("targetSelectionRange").or_else(|| value.get("targetRange"))?,
            ),
            None => (value.get("uri")?.as_str()?, value.get("range")?),
        };
        Some(Place {
            path: path_of(uri)?,
            line: range.pointer("/start/line")?.as_u64()? as u32,
            character: range.pointer("/start/character")?.as_u64()? as u32,
        })
    };
    match answer {
        Value::Array(list) => list.iter().filter_map(one).collect(),
        Value::Object(_) => one(answer).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// Locations, one per line, with the source line next to each. `show` names
/// a path the way the feed does.
pub(crate) fn render_places(
    places: &[Place],
    cap: usize,
    show: &dyn Fn(&Path) -> String,
) -> String {
    let mut sources = Sources::default();
    let mut out = String::new();
    for place in places.iter().take(cap) {
        let (column, snippet) = sources.at(place);
        let _ = write!(out, "{}:{}:{column}", show(&place.path), place.line + 1);
        if let Some(snippet) = snippet {
            let _ = write!(out, "\t{snippet}");
        }
        out.push('\n');
    }
    if places.len() > cap {
        let _ = writeln!(out, "[показаны первые {cap} из {}]", places.len());
    }
    out
}

/// The text of a hover answer: a string, a `{language, value}` pair, a
/// `{kind, value}` markup, or a list of those.
pub(crate) fn hover_text(answer: &Value) -> Option<String> {
    fn part(value: &Value) -> Option<String> {
        match value {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => o.get("value").and_then(Value::as_str).map(str::to_string),
            Value::Array(list) => {
                let parts: Vec<String> = list.iter().filter_map(part).collect();
                (!parts.is_empty()).then(|| parts.join("\n\n"))
            }
            _ => None,
        }
    }
    let text = part(answer.get("contents")?)?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

const KINDS: [&str; 26] = [
    "file", "module", "namespace", "package", "class", "method", "property", "field",
    "constructor", "enum", "interface", "function", "variable", "constant", "string",
    "number", "boolean", "array", "object", "key", "null", "enum member", "struct",
    "event", "operator", "type parameter",
];

fn kind_name(symbol: &Value) -> &'static str {
    symbol
        .get("kind")
        .and_then(Value::as_u64)
        .and_then(|k| KINDS.get((k as usize).wrapping_sub(1)))
        .copied()
        .unwrap_or("symbol")
}

/// The symbols of one file, as an outline: a tree when the server sends
/// `DocumentSymbol`, flat when it sends `SymbolInformation`.
pub(crate) fn render_outline(answer: &Value, cap: usize) -> (String, usize) {
    fn walk(list: &[Value], depth: usize, out: &mut String, shown: &mut usize, cap: usize, total: &mut usize) {
        for symbol in list {
            *total += 1;
            let line = symbol
                .pointer("/selectionRange/start/line")
                .or_else(|| symbol.pointer("/range/start/line"))
                .or_else(|| symbol.pointer("/location/range/start/line"))
                .and_then(Value::as_u64)
                .map_or(0, |l| l + 1);
            if *shown < cap {
                *shown += 1;
                let name = symbol.get("name").and_then(Value::as_str).unwrap_or("?");
                let _ = writeln!(
                    out,
                    "{}{} {name}  :{line}",
                    "  ".repeat(depth),
                    kind_name(symbol)
                );
            }
            if let Some(children) = symbol.get("children").and_then(Value::as_array) {
                walk(children, depth + 1, out, shown, cap, total);
            }
        }
    }
    let mut out = String::new();
    let (mut shown, mut total) = (0, 0);
    if let Some(list) = answer.as_array() {
        walk(list, 0, &mut out, &mut shown, cap, &mut total);
    }
    if total > shown {
        let _ = writeln!(out, "[показаны первые {shown} из {total}]");
    }
    (out, total)
}

/// Symbols found by name across the project.
pub(crate) fn render_found(
    answer: &Value,
    cap: usize,
    show: &dyn Fn(&Path) -> String,
) -> (String, usize) {
    let list = answer.as_array().cloned().unwrap_or_default();
    let mut out = String::new();
    for symbol in list.iter().take(cap) {
        let name = symbol.get("name").and_then(Value::as_str).unwrap_or("?");
        let place = symbol.get("location").and_then(|location| {
            let uri = location.get("uri")?.as_str()?;
            let line = location.pointer("/range/start/line").and_then(Value::as_u64);
            Some((path_of(uri)?, line.map(|l| l + 1)))
        });
        let _ = write!(out, "{} {name}", kind_name(symbol));
        if let Some(container) = symbol
            .get("containerName")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
        {
            let _ = write!(out, " ({container})");
        }
        if let Some((path, line)) = place {
            let _ = write!(out, "  {}", show(&path));
            if let Some(line) = line {
                let _ = write!(out, ":{line}");
            }
        }
        out.push('\n');
    }
    if list.len() > cap {
        let _ = writeln!(out, "[показаны первые {cap} из {}]", list.len());
    }
    (out, list.len())
}

/// How serious a diagnostic is. LSP numbers them 1 to 4; a server that says
/// nothing means an error.
fn severity(diagnostic: &Value) -> u64 {
    diagnostic.get("severity").and_then(Value::as_u64).unwrap_or(1)
}

/// Diagnostics of `path`, worst first. Only those at or above `worst`
/// (1 = errors only) are listed. Returns the text and how many were left out.
pub(crate) fn render_diagnostics(
    diagnostics: &[Value],
    path: &str,
    source: &Path,
    worst: u64,
    cap: usize,
    message_chars: usize,
) -> (String, usize) {
    let mut kept: Vec<&Value> = diagnostics.iter().filter(|d| severity(d) <= worst).collect();
    kept.sort_by_key(|d| {
        (
            severity(d),
            d.pointer("/range/start/line").and_then(Value::as_u64).unwrap_or(0),
        )
    });
    let mut sources = Sources::default();
    let mut out = String::new();
    for diagnostic in kept.iter().take(cap) {
        let place = Place {
            path: source.to_path_buf(),
            line: diagnostic.pointer("/range/start/line").and_then(Value::as_u64).unwrap_or(0) as u32,
            character: diagnostic
                .pointer("/range/start/character")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
        };
        let (column, _) = sources.at(&place);
        let label = match severity(diagnostic) {
            1 => "error",
            2 => "warning",
            3 => "info",
            _ => "hint",
        };
        let message = diagnostic.get("message").and_then(Value::as_str).unwrap_or("");
        let message = message.trim();
        let cut: String = message.chars().take(message_chars).collect();
        let ellipsis = if message.chars().count() > message_chars { "…" } else { "" };
        let code = match diagnostic.get("code") {
            Some(Value::String(code)) => Some(code.clone()),
            Some(Value::Number(code)) => Some(code.to_string()),
            _ => None,
        };
        let by = diagnostic.get("source").and_then(Value::as_str);
        let tag = match (by, code) {
            (Some(by), Some(code)) => format!(" [{by} {code}]"),
            (Some(by), None) => format!(" [{by}]"),
            (None, Some(code)) => format!(" [{code}]"),
            (None, None) => String::new(),
        };
        let _ = writeln!(out, "{label} {path}:{}:{column} {cut}{ellipsis}{tag}", place.line + 1);
    }
    (out, kept.len().saturating_sub(cap))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const SRC: &str = "fn main() {\n    let id = valid(id);\n    ёж(x, x)\n}\n";

    #[test]
    fn a_symbol_is_found_by_its_text_and_counted_from_zero() {
        // `valid` on line 2: four spaces, `let id = ` is 9 characters.
        assert_eq!(locate(SRC, 2, Some("valid"), None), Ok((1, 13)));
        assert_eq!(locate(SRC, 1, None, None), Ok((0, 0)));
        assert_eq!(locate(SRC, 2, None, None), Ok((1, 4)));
    }

    #[test]
    fn a_whole_word_wins_over_the_end_of_a_longer_one() {
        // `id` is also inside `valid`'s neighbour text; the standalone ones are
        // `id` after `let` and `id` in the call: two, so it is ambiguous...
        let err = locate(SRC, 2, Some("id"), None).unwrap_err();
        assert!(err.contains("несколько раз"), "{err}");
        // ...but `valid(id` is one place.
        assert_eq!(locate(SRC, 2, Some("valid(id"), None), Ok((1, 13)));
    }

    #[test]
    fn the_column_counts_utf16_units_not_characters() {
        // `😀` is two units, `ё` one: the text after them starts later in
        // units than in characters.
        let text = "😀ёx = 1\n";
        assert_eq!(locate(text, 1, Some("x"), None), Ok((0, 3)));
    }

    #[test]
    fn a_line_outside_the_file_says_how_long_the_file_is() {
        let err = locate(SRC, 9, None, None).unwrap_err();
        assert!(err.contains("4 строк"), "{err}");
        assert!(locate(SRC, 0, None, None).is_err());
    }

    #[test]
    fn a_missing_symbol_shows_the_line_it_was_looked_for_in() {
        let err = locate(SRC, 3, Some("zzz"), None).unwrap_err();
        assert!(err.contains("ёж(x, x)"), "{err}");
    }

    #[test]
    fn locations_come_in_all_three_shapes() {
        let range = json!({ "start": { "line": 4, "character": 2 }, "end": { "line": 4, "character": 5 } });
        let one = json!({ "uri": "file:///p/a.rs", "range": range });
        let link = json!({ "targetUri": "file:///p/b.rs", "targetRange": range, "targetSelectionRange": range });
        assert_eq!(places(&one).len(), 1);
        assert_eq!(places(&json!([one, link])).len(), 2);
        assert_eq!(places(&json!([link]))[0].path, PathBuf::from("/p/b.rs"));
        assert!(places(&Value::Null).is_empty());
    }

    #[test]
    fn hover_comes_as_a_string_markup_or_a_list() {
        assert_eq!(hover_text(&json!({ "contents": "fn x()" })).as_deref(), Some("fn x()"));
        assert_eq!(
            hover_text(&json!({ "contents": { "kind": "markdown", "value": "```rust\nfn x()\n```" } }))
                .as_deref(),
            Some("```rust\nfn x()\n```")
        );
        assert_eq!(
            hover_text(&json!({ "contents": [{ "language": "rust", "value": "a" }, "b"] })).as_deref(),
            Some("a\n\nb")
        );
        assert_eq!(hover_text(&json!({ "contents": "  " })), None);
    }

    #[test]
    fn an_outline_keeps_the_tree_and_counts_what_it_cut() {
        let answer = json!([{
            "name": "Foo", "kind": 23,
            "selectionRange": { "start": { "line": 0, "character": 0 } },
            "children": [
                { "name": "bar", "kind": 6, "selectionRange": { "start": { "line": 2, "character": 0 } } },
                { "name": "baz", "kind": 6, "selectionRange": { "start": { "line": 5, "character": 0 } } },
            ],
        }]);
        let (text, total) = render_outline(&answer, 2);
        assert_eq!(total, 3);
        assert!(text.starts_with("struct Foo  :1\n  method bar  :3\n"), "{text}");
        assert!(text.contains("показаны первые 2 из 3"));
    }

    #[test]
    fn diagnostics_list_worst_first_and_leave_out_the_milder() {
        let list = vec![
            json!({ "severity": 2, "message": "unused", "range": { "start": { "line": 0, "character": 0 } } }),
            json!({ "severity": 1, "message": "mismatched types", "source": "rustc", "code": "E0308",
                    "range": { "start": { "line": 6, "character": 4 } } }),
        ];
        let (text, rest) = render_diagnostics(&list, "src/a.rs", Path::new("/nope"), 1, 10, 300);
        assert_eq!(text, "error src/a.rs:7:5 mismatched types [rustc E0308]\n");
        assert_eq!(rest, 0);

        let (text, _) = render_diagnostics(&list, "src/a.rs", Path::new("/nope"), 2, 10, 300);
        assert!(text.lines().next().unwrap().starts_with("error"));
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn a_long_message_is_cut_with_a_mark() {
        let list = vec![json!({ "message": "x".repeat(50), "range": { "start": { "line": 0, "character": 0 } } })];
        let (text, _) = render_diagnostics(&list, "a", Path::new("/nope"), 4, 10, 10);
        assert!(text.contains("xxxxxxxxxx…"), "{text}");
    }
}
