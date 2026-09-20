//! Удаление и перемещение файла.
//!
//! Read-before-edit здесь намеренно не требуется. Инвариант защищает от
//! правки вслепую — от того, что модель затрёт код, которого не видела.
//! Удаление ничего не затирает поверх, а безопасность даёт §8: и удаление,
//! и перемещение спрашивают разрешения везде, кроме «Авто».

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files;
use super::{FileChange, Narration, Tool, ToolCtx, ToolOutput};
use crate::change::{ChangeKind, FileFact};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::{PermissionKind, PermissionNeed};
use crate::workspace::Workspace;

pub struct DeleteFile;

#[async_trait]
impl Tool for DeleteFile {
    fn name(&self) -> &'static str {
        "Delete"
    }

    fn description(&self) -> &'static str {
        "Delete a file."
    }

    fn properties(&self) -> Value {
        json!({
            "path": { "type": "string", "description": "Path relative to the project root." },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["path"]
    }

    fn narration(&self) -> Narration {
        Narration::Interface
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["path"].as_str().map(str::to_owned)
    }

    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let raw = input["path"].as_str()?;
        let abs = ws.resolve_new(raw).ok()?;
        let need = PermissionNeed::new(PermissionKind::DeleteFile, self.name(), short(&abs))
            .detail(ws.display(&abs))
            .path(abs.clone());
        Some(match std::fs::metadata(&abs) {
            Ok(meta) => need.size_bytes(meta.len()),
            Err(_) => need,
        })
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан path"))?;

        let target = files::look(ctx, raw)?;
        if target.abs.is_dir() {
            return Err(ToolError::failed(format!(
                "{raw} — каталог; его удаляет DeleteDir"
            )));
        }
        if target.current.is_none() {
            return Err(ToolError::failed(format!("не найдено: {raw}")));
        }

        let rel = target.rel.clone();
        let (fact, diff) = files::write(ctx, &target, ChangeKind::Deleted, None)?;

        Ok(ToolOutput::new(format!("Удалён {rel}\n")).changed(FileChange { fact, diff }))
    }
}

pub struct MoveFile;

#[async_trait]
impl Tool for MoveFile {
    fn name(&self) -> &'static str {
        "Move"
    }

    fn description(&self) -> &'static str {
        "Move or rename a file. The content is not changed."
    }

    fn properties(&self) -> Value {
        json!({
            "from": { "type": "string", "description": "Current path." },
            "to": { "type": "string", "description": "New path." },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["from", "to"]
    }

    fn narration(&self) -> Narration {
        Narration::Interface
    }

    fn intents(&self) -> &'static [Intent] {
        &[Intent::Agent]
    }

    fn target(&self, input: &Value) -> Option<String> {
        match (input["from"].as_str(), input["to"].as_str()) {
            (Some(from), Some(to)) => Some(format!("{from} → {to}")),
            _ => None,
        }
    }

    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let from = ws.resolve_new(input["from"].as_str()?).ok()?;
        let to = ws.resolve_new(input["to"].as_str()?).ok()?;
        Some(
            PermissionNeed::new(PermissionKind::MoveFile, self.name(), short(&from))
                .detail(format!("{} → {}", ws.display(&from), ws.display(&to)))
                .path(from),
        )
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_from = input["from"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан from"))?;
        let raw_to = input["to"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан to"))?;

        let from = files::look(ctx, raw_from)?;
        let to = files::look(ctx, raw_to)?;

        if from.current.is_none() {
            return Err(ToolError::failed(format!("не найдено: {raw_from}")));
        }
        if to.current.is_some() {
            return Err(ToolError::failed(format!(
                "{raw_to} уже существует — перемещение затёрло бы его"
            )));
        }
        if from.abs == to.abs {
            return Err(ToolError::failed("from и to совпадают"));
        }

        if let Some(parent) = to.abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::failed(e.to_string()))?;
        }
        std::fs::rename(&from.abs, &to.abs).map_err(|e| ToolError::failed(e.to_string()))?;
        ctx.ws.forget(&from.abs);
        ctx.ws.mark_read(&to.abs);

        // Содержимое не менялось — дифф пуст, а перемещение подписывает
        // интерфейс по факту (§5.5).
        let fact = FileFact {
            path: to.rel.clone(),
            kind: ChangeKind::Moved,
            moved_from: Some(from.rel.clone()),
            added: 0,
            removed: 0,
        };

        Ok(ToolOutput::new(format!(
            "Перемещён {} → {}\n",
            from.rel, to.rel
        ))
        .changed(FileChange {
            fact,
            diff: Default::default(),
        }))
    }
}

/// Имя файла без пути — второй ярус запроса разрешения (§5.7).
fn short(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::ChangeKind;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn удаление_уносит_файл_с_диска() {
        let dir = TempDir::new();
        let path = dir.write("лишний.rs", "тело\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = DeleteFile
            .run(&ctx, json!({ "path": "лишний.rs" }))
            .await
            .unwrap();

        assert_eq!(out.changed.unwrap().fact.kind, ChangeKind::Deleted);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn удалённый_файл_больше_не_читается() {
        let dir = TempDir::new();
        dir.write("a.rs", "тело\n");
        let (ctx, _ws) = ctx_for(&dir);

        DeleteFile
            .run(&ctx, json!({ "path": "a.rs" }))
            .await
            .unwrap();

        let err = DeleteFile
            .run(&ctx, json!({ "path": "a.rs" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("не найдено"));
    }

    #[tokio::test]
    async fn перемещение_даёт_одну_строку_а_не_две() {
        let dir = TempDir::new();
        let from = dir.write("было.rs", "тело\n");
        let (ctx, _ws) = ctx_for(&dir);

        let out = MoveFile
            .run(&ctx, json!({ "from": "было.rs", "to": "src/стало.rs" }))
            .await
            .unwrap();

        let fact = out.changed.unwrap().fact;
        assert_eq!(fact.kind, ChangeKind::Moved);
        assert_eq!(fact.moved_from.as_deref(), Some("было.rs"));
        assert!(!from.exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/стало.rs")).unwrap(),
            "тело\n"
        );
    }

    #[tokio::test]
    async fn перемещение_поверх_существующего_файла_запрещено() {
        let dir = TempDir::new();
        dir.write("a.rs", "первый\n");
        dir.write("b.rs", "второй\n");
        let (ctx, _ws) = ctx_for(&dir);

        let err = MoveFile
            .run(&ctx, json!({ "from": "a.rs", "to": "b.rs" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("уже существует"));
    }
}
