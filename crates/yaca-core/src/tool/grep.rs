//! Поиск по содержимому. Регулярное выражение, обход с уважением к `.gitignore`.

mod excerpts;

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use excerpts::Excerpts;

/// Файлы крупнее — почти наверняка данные, а не код.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Сколько поиску позволено идти, прежде чем его остановят.
///
/// Обход уважает `.gitignore`, но каталог без него — симлинк в корень диска,
/// сетевой том, дерево на миллион файлов — уводит поиск в минуты. Строка
/// действия при этом просто висит, и человек видит зависшее приложение, а не
/// работающий поиск (§0). Минуты хватает всему, что вообще имеет смысл ждать.
const DEADLINE: Duration = Duration::from_secs(60);

pub struct Grep {
    /// Вынесен в поле, чтобы тест мог поставить свой: ждать минуту, проверяя
    /// минутный потолок, — не тест, а простой.
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
         groups, which often saves reading the file. Mode `files` returns only paths."
    }

    fn properties(&self) -> Value {
        json!({
            "pattern": {
                "type": "string",
                "description": "Regular expression in Rust regex syntax.",
            },
            "path": {
                "type": "string",
                "description": "Directory or file to search in. Defaults to the entire project.",
            },
            "glob": {
                "type": "string",
                "description": "Restrict to a path pattern, e.g. `**/*.rs`.",
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

    /// Поиск ничего не меняет.
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
            .ok_or_else(|| ToolError::failed("не задан pattern"))?;
        let re = regex::RegexBuilder::new(pattern)
            .case_insensitive(input["case_insensitive"].as_bool().unwrap_or(false))
            .build()
            .map_err(|e| ToolError::failed(format!("плохое выражение `{pattern}`: {e}")))?;

        let start = match input["path"].as_str() {
            Some(p) => ctx.ws.resolve_existing(p)?,
            None => ctx.ws.root().to_path_buf(),
        };

        let path_filter = match input["glob"].as_str() {
            Some(g) => Some(
                globset::GlobBuilder::new(g)
                    .literal_separator(true)
                    .build()
                    .map_err(|e| ToolError::failed(format!("плохой шаблон `{g}`: {e}")))?
                    .compile_matcher(),
            ),
            None => None,
        };

        let files_only = input["mode"].as_str() == Some("files");
        let lines_of = |key: &str| input[key].as_u64().map(|n| n as usize);
        let around = lines_of("context").unwrap_or(0);
        let mut excerpts = Excerpts::new(
            lines_of("before").unwrap_or(around),
            lines_of("after").unwrap_or(around),
        );
        let ws = ctx.ws.clone();
        let cancel = ctx.cancel.clone();
        let limit = self.deadline;
        let deadline = Instant::now() + limit;

        let (excerpts, files, total) = tokio::task::spawn_blocking(move || {
            let mut files: Vec<String> = Vec::new();
            let mut total = 0usize;

            // Сначала собираем, что вообще читать, потом ищем: обход и чтение
            // не мешаются в одном горячем цикле.
            let mut targets: Vec<(String, std::path::PathBuf)> = Vec::new();

            for entry in super::files::walk(&start) {
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
                let rel = ws.display(entry.path());
                if path_filter.as_ref().is_some_and(|m| !m.is_match(&rel)) {
                    continue;
                }
                targets.push((rel, entry.path().to_path_buf()));
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
                    continue; // двоичный
                }
                let text = String::from_utf8_lossy(&bytes).into_owned();

                if files_only {
                    if text.lines().any(|line| re.is_match(line)) {
                        total += 1;
                        files.push(rel);
                    }
                    continue;
                }

                // Context needs the lines around a match, so the file is
                // split once and the matches are found by index.
                let lines: Vec<&str> = text.lines().collect();
                let hits: Vec<usize> = (0..lines.len()).filter(|&i| re.is_match(lines[i])).collect();
                if hits.is_empty() {
                    continue;
                }
                total += hits.len();
                excerpts.file(&rel, &lines, &hits);
                files.push(rel);
            }
            Ok((excerpts, files, total))
        })
        .await
        .map_err(|e| ToolError::failed(format!("поиск упал: {e}")))??;

        let mut out = String::new();
        let truncated;
        let shown;
        if files_only {
            truncated = false;
            shown = files.len();
            for f in &files {
                out.push_str(f);
                out.push('\n');
            }
        } else {
            let (text, matches) = excerpts.finish();
            out = text;
            shown = matches;
            truncated = total > shown;
        }
        if total == 0 {
            out.push_str("[совпадений нет]\n");
        } else if truncated {
            out.push_str(&format!(
                "\n[показаны {shown} из {total} совпадений в {} файлах]\n",
                files.len()
            ));
        }

        let items = if files_only { files.len() } else { total };
        Ok(ToolOutput::new(out)
            .items(items as u32)
            .truncated(truncated))
    }
}

/// Форма неудачи для ленты и для модели. Красная (§5.9 красит `Failed`
/// `--rust`): поиск не довели до конца, и его результат — не «ничего не
/// нашлось», а «неизвестно».
fn overran(limit: Duration) -> ToolError {
    ToolError::failed_with(
        format!("поиск не уложился в {} с и остановлен", limit.as_secs()),
        "Полный обход проекта в отведённое время не укладывается. Ищи в подкаталоге или сузь `glob`.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    /// The walk is stopped by the clock, and it says so in red: a search that
    /// did not finish did not find "nothing", it found nothing *yet* (§5.9).
    #[tokio::test]
    async fn a_search_that_overruns_is_stopped_and_says_so() {
        let dir = TempDir::new();
        dir.write(
            "a.rs",
            "найди меня
",
        );
        let (ctx, _ws) = ctx_for(&dir);

        let tool = Grep {
            deadline: Duration::ZERO,
        };
        let err = tool
            .run(&ctx, json!({ "pattern": "найди" }))
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Failed { .. }), "получили {err:?}");
        assert!(err.to_string().contains("не уложился"), "{err}");
        // The model gets the way out as well, not only the fact.
        assert!(
            err.for_model().contains("подкаталоге"),
            "{}",
            err.for_model()
        );
    }
    #[tokio::test]
    async fn находит_строки_с_номерами() {
        let dir = TempDir::new();
        dir.write("src/a.rs", "let x = 1;\nlet clipboard = 2;\n");
        dir.write("src/b.ts", "const clipboard = 3;\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(&ctx, json!({ "pattern": "clipboard" }))
            .await
            .unwrap();

        assert!(out.text.contains("src/a.rs:2: let clipboard = 2;"));
        assert!(out.text.contains("src/b.ts:1:"));
        assert_eq!(out.meta.items, Some(2));
    }

    #[tokio::test]
    async fn шаблон_пути_сужает_поиск() {
        let dir = TempDir::new();
        dir.write("src/a.rs", "clipboard\n");
        dir.write("src/b.ts", "clipboard\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(&ctx, json!({ "pattern": "clipboard", "glob": "**/*.rs" }))
            .await
            .unwrap();

        assert!(out.text.contains("src/a.rs"));
        assert!(!out.text.contains("b.ts"));
    }

    #[tokio::test]
    async fn режим_files_отдаёт_только_пути() {
        let dir = TempDir::new();
        dir.write("a.rs", "нужное\nнужное\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(&ctx, json!({ "pattern": "нужное", "mode": "files" }))
            .await
            .unwrap();

        assert_eq!(out.text.trim(), "a.rs");
        assert_eq!(out.meta.items, Some(1));
    }

    #[tokio::test]
    async fn игнор_действует_и_вне_git_репозитория() {
        let dir = TempDir::new();
        dir.write(
            ".gitignore",
            "target
node_modules
",
        );
        dir.write(
            "target/debug/build.rs",
            "нужное
",
        );
        dir.write(
            "node_modules/pkg/index.js",
            "нужное
",
        );
        dir.write(
            "src/a.rs",
            "нужное
",
        );
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(&ctx, json!({ "pattern": "нужное" }))
            .await
            .unwrap();

        assert!(out.text.contains("src/a.rs"), "получили: {}", out.text);
        assert!(!out.text.contains("target"), "получили: {}", out.text);
        assert!(!out.text.contains("node_modules"), "получили: {}", out.text);
    }

    fn numbered(n: usize) -> String {
        (1..=n).map(|i| format!("line {i}\n")).collect()
    }

    #[tokio::test]
    async fn context_lines_surround_a_match_and_are_marked_apart() {
        let dir = TempDir::new();
        dir.write("a.rs", "one\ntwo\nfound\nfour\nfive\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(&ctx, json!({ "pattern": "found", "context": 1 }))
            .await
            .unwrap();

        // ripgrep's marks: `:` on the match, `-` on the lines around it.
        assert_eq!(out.text, "a.rs-2- two\na.rs:3: found\na.rs-4- four\n");
        assert_eq!(out.meta.items, Some(1), "context lines are not matches");
    }

    #[tokio::test]
    async fn before_and_after_are_separate_and_override_context() {
        let dir = TempDir::new();
        dir.write("a.rs", &numbered(9));
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(
                &ctx,
                json!({ "pattern": "^line 5$", "context": 3, "before": 2, "after": 0 }),
            )
            .await
            .unwrap();

        assert_eq!(out.text, "a.rs-3- line 3\na.rs-4- line 4\na.rs:5: line 5\n");
    }

    #[tokio::test]
    async fn overlapping_windows_merge_and_distant_ones_are_split() {
        let dir = TempDir::new();
        dir.write("a.rs", &numbered(12));
        let (ctx, _ws) = ctx_for(&dir);

        // Matches on 2, 4 and 10: the first two windows share line 3.
        let out = Grep::new()
            .run(
                &ctx,
                json!({ "pattern": "^line (2|4|10)$", "context": 1 }),
            )
            .await
            .unwrap();

        assert_eq!(
            out.text,
            "a.rs-1- line 1\na.rs:2: line 2\na.rs-3- line 3\na.rs:4: line 4\na.rs-5- line 5\n\
             --\n\
             a.rs-9- line 9\na.rs:10: line 10\na.rs-11- line 11\n"
        );
        assert_eq!(out.meta.items, Some(3));
    }

    #[tokio::test]
    async fn groups_in_different_files_are_split_too() {
        let dir = TempDir::new();
        dir.write("a.rs", "x\nfound\n");
        dir.write("b.rs", "found\ny\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(&ctx, json!({ "pattern": "found", "context": 1 }))
            .await
            .unwrap();

        assert_eq!(out.text, "a.rs-1- x\na.rs:2: found\n--\nb.rs:1: found\nb.rs-2- y\n");
    }

    #[tokio::test]
    async fn wide_context_is_cut_to_a_budget_and_counted_in_matches() {
        let dir = TempDir::new();
        let wide = "w".repeat(300);
        let text: String = (0..2_000)
            .map(|i| if i % 10 == 0 { "hit\n".to_string() } else { format!("{wide}\n") })
            .collect();
        dir.write("a.rs", &text);
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(&ctx, json!({ "pattern": "^hit$", "context": 9 }))
            .await
            .unwrap();

        // Two hundred matches with nine lines of context each way would be
        // well over a megabyte.
        assert!(out.text.len() < 110_000, "{} bytes", out.text.len());
        assert!(out.meta.truncated);
        // The footer counts matches, not the lines around them.
        let footer = out.text.lines().last().unwrap();
        assert!(footer.contains("из 200 совпадений"), "{footer}");
    }

    #[tokio::test]
    async fn отсутствие_совпадений_проговаривается() {
        let dir = TempDir::new();
        dir.write("a.rs", "ничего\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Grep::new()
            .run(&ctx, json!({ "pattern": "^нет$" }))
            .await
            .unwrap();
        assert!(out.text.contains("совпадений нет"));
    }
}
