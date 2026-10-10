//! Чтение файла. Подпись пишет модель (§5.4.1): зачем читали, интерфейс не знает.
//!
//! Читает сквозь набор непринятых правок. Иначе агент, только что изменивший
//! файл, перечитает его, увидит старое содержимое и начнёт спорить сам с собой.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files;
use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;

/// Потолок за один вызов. Больше — только явным `offset`/`limit`:
/// иначе один `read_file` съедает окно контекста.
const DEFAULT_LIMIT: usize = 2_000;
/// Длинные строки режутся: минифицированный бандл в одну строку способен
/// выбить окно целиком.
const MAX_LINE_CHARS: usize = 2_000;
/// How much of the file one call hands the model, in bytes, line numbers
/// included. The two limits above do not bound it: two thousand lines of a
/// wide CSV are two megabytes, more than a whole context window from one call,
/// and it stays in the window for every request after. Sized so that the
/// default window of ordinary code — about forty characters a line — still
/// arrives whole: roughly 25k tokens.
const MAX_OUTPUT_BYTES: usize = 100_000;

pub struct ReadFile;

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &'static str {
        "Read"
    }

    fn description(&self) -> &'static str {
        "Read a project file. Returns numbered lines. \
         For large files, use offset and limit."
    }

    fn properties(&self) -> Value {
        json!({
            "path": {
                "type": "string",
                "description": "Path relative to the project root.",
            },
            "offset": {
                "type": "integer",
                "minimum": 1,
                "description": "Line number to start from, 1-indexed.",
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "description": "Number of lines to read.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Чтение ничего не меняет и ни о чём не спрашивает.
    fn concurrent(&self) -> bool {
        true
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Plan, Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_path = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан path"))?;

        // An image read as text is two megabytes of noise in the window, and
        // "binary file" would leave the model with nowhere to go next.
        if ctx.ws.resolve_new(raw_path).is_ok_and(|abs| crate::picture::is_picture(&abs)) {
            return Err(ToolError::failed_with(
                format!("{raw_path} — изображение, а не текст"),
                "This file is an image. Look at it with ReadImage.",
            ));
        }

        // Чтение уходит на блокирующий исполнитель: большой файл на сетевом
        // диске иначе останавливает всю сессию — вместе с каналом событий и
        // отменой, — а в пачке §5.4 держит соседние чтения, которые должны
        // были идти рядом.
        let ws = ctx.ws.clone();
        let raw = raw_path.to_string();
        let target = tokio::task::spawn_blocking(move || files::look_in(&ws, &raw))
            .await
            .map_err(|_| ToolError::failed("чтение файла не завершилось"))??;

        if target.abs.is_dir() {
            return Err(ToolError::failed(format!("{raw_path} — каталог, не файл")));
        }
        let Some(text) = target.current else {
            return Err(ToolError::failed(format!("не найдено: {raw_path}")));
        };

        let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = input["limit"].as_u64().unwrap_or(DEFAULT_LIMIT as u64) as usize;

        let all: Vec<&str> = text.lines().collect();
        let total = all.len();
        let start = offset - 1;

        if start >= total && total > 0 {
            return Err(ToolError::failed(format!(
                "в файле {total} строк, запрошена строка {offset}"
            )));
        }

        let window = (start + limit).min(total);
        let mut out = String::new();
        // The last line shown, which the budget may put before the window's
        // end. The first line always fits: a clipped line is a few kilobytes.
        let mut end = start;
        for (i, line) in all[start..window].iter().enumerate() {
            ctx.check_cancelled()?;
            let n = start + i + 1;
            let shown: String = if line.chars().count() > MAX_LINE_CHARS {
                line.chars().take(MAX_LINE_CHARS).collect::<String>() + " …[строка обрезана]"
            } else {
                (*line).to_string()
            };
            let row = format!("{n:>6}\t{shown}\n");
            if out.len() + row.len() > MAX_OUTPUT_BYTES {
                break;
            }
            out.push_str(&row);
            end = n;
        }

        // Файл прочитан — реестр запомнил отпечаток (§8, read-before-edit).
        // Для файла, который существует только в наборе, отпечатка нет:
        // сверять его будет не с чем, и это правильно.
        if ctx.ws.fingerprint(&target.abs).is_some() {
            ctx.ws.mark_read(&target.abs);
        } else {
            ctx.ws.mark_authored(&target.abs);
        }

        let truncated = end < total;
        if truncated {
            out.push_str(&format!(
                "\n[показаны строки {}–{} из {total}]\n",
                start + 1,
                end
            ));
        }
        if total == 0 {
            out.push_str("[файл пуст]\n");
        }

        Ok(ToolOutput::new(out)
            .items(total as u32)
            .truncated(truncated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn нумерует_строки_и_помечает_прочитанным() {
        let dir = TempDir::new();
        dir.write("a.rs", "первая\nвторая\nтретья\n");
        let (ctx, ws) = ctx_for(&dir);

        let out = ReadFile.run(&ctx, json!({ "path": "a.rs" })).await.unwrap();

        assert!(out.text.contains("     1\tпервая"));
        assert!(out.text.contains("     3\tтретья"));
        assert_eq!(out.meta.items, Some(3));
        assert!(!out.meta.truncated);
        assert!(ws.was_read(&ws.resolve_existing("a.rs").unwrap()));
    }

    #[tokio::test]
    async fn окно_по_offset_и_limit() {
        let dir = TempDir::new();
        dir.write("a.rs", "1\n2\n3\n4\n5\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadFile
            .run(&ctx, json!({ "path": "a.rs", "offset": 2, "limit": 2 }))
            .await
            .unwrap();

        assert!(out.text.contains("     2\t2"));
        assert!(out.text.contains("     3\t3"));
        assert!(!out.text.contains("\t4"));
        assert!(out.meta.truncated);
    }

    #[tokio::test]
    async fn a_wide_file_is_cut_to_a_budget_and_says_where() {
        let dir = TempDir::new();
        let line = "x".repeat(1_000);
        dir.write("wide.csv", &format!("{line}\n").repeat(2_000));
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadFile.run(&ctx, json!({ "path": "wide.csv" })).await.unwrap();

        // Two thousand lines of a thousand characters is two megabytes — more
        // than a whole context window, from one call.
        assert!(out.text.len() <= MAX_OUTPUT_BYTES + 100, "{} bytes", out.text.len());
        assert!(out.meta.truncated);
        assert!(!out.text.contains("  2000\t"));
        // The footer is how the model finds out where to go on from.
        assert!(out.text.contains("из 2000]"), "{:?}", out.text.lines().last());
    }

    #[tokio::test]
    async fn an_ordinary_file_of_the_default_window_still_fits_whole() {
        let dir = TempDir::new();
        let line = format!("    let value = compute(input, &config);{}", " ");
        dir.write("a.rs", &format!("{line}\n").repeat(2_000));
        let (ctx, _ws) = ctx_for(&dir);

        let out = ReadFile.run(&ctx, json!({ "path": "a.rs" })).await.unwrap();

        assert!(!out.meta.truncated);
        assert!(out.text.contains("  2000\t"));
    }

    #[tokio::test]
    async fn выход_за_проект_это_отклонение_а_не_сбой() {
        let dir = TempDir::new();
        dir.write("a.rs", "x");
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadFile
            .run(&ctx, json!({ "path": "../снаружи.rs" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "получили {err:?}");
    }

    #[tokio::test]
    async fn двоичный_файл_отвергается_понятно() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("bin"), [0u8, 1, 2, 3]).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadFile
            .run(&ctx, json!({ "path": "bin" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("двоичный"));
    }

    #[tokio::test]
    async fn an_image_is_sent_to_the_tool_that_can_show_it() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join("shot.png"), crate::picture::tests::png(8, 8)).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadFile
            .run(&ctx, json!({ "path": "shot.png" }))
            .await
            .unwrap_err();
        assert!(err.for_model().contains("ReadImage"), "{}", err.for_model());
    }

    #[tokio::test]
    async fn несуществующий_файл_это_понятный_отказ() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);

        let err = ReadFile
            .run(&ctx, json!({ "path": "нет-такого.rs" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("не найдено"));
    }
}
