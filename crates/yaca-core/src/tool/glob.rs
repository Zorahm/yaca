//! Поиск файлов по шаблону. Обход уважает `.gitignore`: агенту нечего делать
//! в `node_modules`, и подмешивать туда внимание модели — прямой убыток.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;

const MAX_RESULTS: usize = 200;

pub struct Glob;

#[async_trait]
impl Tool for Glob {
    fn name(&self) -> &'static str {
        "Glob"
    }

    fn description(&self) -> &'static str {
        "Find files matching a path pattern, e.g. `src/**/*.rs`. \
         Results are sorted by modification time, freshest first."
    }

    fn properties(&self) -> Value {
        json!({
            "pattern": {
                "type": "string",
                "description": "Path pattern relative to the project root. In an additional \
                                root it is matched against the path inside that root.",
            },
            "path": {
                "type": "string",
                "description": "Directory to start searching from. Defaults to every root, \
                                the project first. Files in an additional root are shown \
                                by absolute path.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["pattern"]
    }

    fn narration(&self) -> Narration {
        Narration::Model
    }

    /// Обход дерева ничего не меняет.
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
            .ok_or_else(|| ToolError::failed("не задан pattern"))?
            .to_string();
        let given = match input["path"].as_str() {
            Some(p) => Some(ctx.ws.resolve_existing(p)?),
            None => None,
        };
        let starts = super::files::starts(&ctx.ws, given);

        let matcher = globset::GlobBuilder::new(&pattern)
            .literal_separator(true)
            .build()
            .map_err(|e| ToolError::failed(format!("плохой шаблон `{pattern}`: {e}")))?
            .compile_matcher();

        let ws = ctx.ws.clone();
        let cancel = ctx.cancel.clone();

        // Обход синхронный и может быть долгим — уводим с реактора.
        let found = tokio::task::spawn_blocking(move || {
            let mut hits: Vec<(u64, String)> = Vec::new();
            for start in &starts {
                for entry in super::files::walk(start) {
                    if cancel.is_cancelled() {
                        return Err(ToolError::Cancelled);
                    }
                    let Ok(entry) = entry else { continue };
                    if !entry.file_type().is_some_and(|t| t.is_file()) {
                        continue;
                    }
                    // Matched inside the root that holds the file, shown as
                    // the root form (absolute for an additional root).
                    if !matcher.is_match(ws.within_root(entry.path())) {
                        continue;
                    }
                    let mtime = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    hits.push((mtime, ws.display(entry.path())));
                }
            }

            hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            hits.dedup_by(|a, b| a.1 == b.1);
            Ok(hits)
        })
        .await
        .map_err(|e| ToolError::failed(format!("обход упал: {e}")))??;

        let total = found.len();
        let truncated = total > MAX_RESULTS;
        let shown = found.iter().take(MAX_RESULTS);

        let mut out = String::new();
        for (_, rel) in shown {
            out.push_str(rel);
            out.push('\n');
        }
        if total == 0 {
            out.push_str("[ничего не найдено]\n");
        }
        if truncated {
            out.push_str(&format!("\n[показаны {MAX_RESULTS} из {total}]\n"));
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
    async fn находит_по_шаблону_и_не_лезет_в_игнор() {
        let dir = TempDir::new();
        dir.write(".gitignore", "node_modules\n");
        dir.write("src/a.rs", "");
        dir.write("src/deep/b.rs", "");
        dir.write("src/c.ts", "");
        dir.write("node_modules/mod.rs", "");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Glob
            .run(&ctx, json!({ "pattern": "src/**/*.rs" }))
            .await
            .unwrap();

        assert!(out.text.contains("src/a.rs"));
        assert!(out.text.contains("src/deep/b.rs"));
        assert!(!out.text.contains("c.ts"));
        assert!(!out.text.contains("node_modules"));
    }

    #[tokio::test]
    async fn игнор_действует_и_вне_git_репозитория() {
        let dir = TempDir::new();
        dir.write(".gitignore", "node_modules
target
");
        dir.write("src/a.rs", "");
        dir.write("node_modules/pkg/b.rs", "");
        dir.write("target/debug/c.rs", "");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Glob
            .run(&ctx, json!({ "pattern": "**/*.rs" }))
            .await
            .unwrap();

        assert!(out.text.contains("src/a.rs"), "получили: {}", out.text);
        assert!(!out.text.contains("node_modules"), "получили: {}", out.text);
        assert!(!out.text.contains("target"), "получили: {}", out.text);
    }

    #[tokio::test]
    async fn пустой_результат_говорит_об_этом_прямо() {
        let dir = TempDir::new();
        dir.write("a.txt", "");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Glob
            .run(&ctx, json!({ "pattern": "**/*.rs" }))
            .await
            .unwrap();
        assert!(out.text.contains("ничего не найдено"));
        assert_eq!(out.meta.items, Some(0));
    }

    #[tokio::test]
    async fn кривой_шаблон_это_сбой_а_не_паника() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let err = Glob.run(&ctx, json!({ "pattern": "[" })).await.unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }));
    }
}
