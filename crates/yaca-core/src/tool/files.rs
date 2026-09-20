//! Общая обвязка файловых тулов: запись в дерево и инварианты (§8).
//!
//! Инварианты живут здесь, а не в каждом туле, потому что «не отключаются
//! ничем» — это про одно место, а не про четыре одинаковых проверки, из
//! которых одну однажды забудут.

use std::path::{Path, PathBuf};

use crate::change::{ChangeKind, FileDiff, FileFact};
use crate::error::ToolError;
use crate::workspace::{ReadRecord, Workspace};

use super::ToolCtx;

/// Обход дерева для читающих тулов: `grep` и `glob` ходят одинаково.
///
/// `require_git(false)` здесь не украшение. По умолчанию `ignore` применяет
/// `.gitignore` только внутри git-репозитория, а проект без `.git` — обычное
/// дело. Молча получив полный обход, тул уходит в `target/` и `node_modules/`
/// и читает десятки тысяч файлов: строка в ленте висит минутами, и человек
/// видит зависание, а не поиск.
pub(crate) fn walk(start: &Path) -> ignore::Walk {
    ignore::WalkBuilder::new(start)
        .hidden(true)
        .require_git(false)
        .build()
}

/// Цель правки, прошедшая проверки.
pub(crate) struct Target {
    pub abs: PathBuf,
    pub rel: String,
    /// Содержимое на диске. `None` — файла нет.
    pub current: Option<String>,
    /// Отпечаток диска: по нему §8 поймает правку мимо Yaca.
    pub fingerprint: Option<ReadRecord>,
}

/// Читает файл, не проверяя ничего сверх границ проекта.
pub(crate) fn look(ctx: &ToolCtx, raw: &str) -> Result<Target, ToolError> {
    look_in(&ctx.ws, raw)
}

/// То же по воркспейсу, без всего остального контекста.
///
/// Отдельно, потому что это единственная часть чтения, которая ходит на диск,
/// а `ToolCtx` через границу задачи не пронести. Тул, который хочет читать, не
/// занимая исполнитель, уносит вызов в `spawn_blocking` с копией `Arc`.
pub(crate) fn look_in(ws: &Workspace, raw: &str) -> Result<Target, ToolError> {
    let abs = ws.resolve_new(raw)?;
    Ok(Target {
        rel: ws.display(&abs),
        current: read_text(&abs)?,
        fingerprint: ws.fingerprint(&abs),
        abs,
    })
}

/// Записывает содержимое в рабочее дерево и возвращает то, что интерфейс
/// покажет строкой и диффом.
///
/// Отпечаток обновляется здесь же: запись меняет mtime, и без этого агент не
/// смог бы поправить файл дважды подряд — вторая правка упёрлась бы в
/// собственную первую как в «файл изменился после чтения».
pub(crate) fn write(
    ctx: &ToolCtx,
    target: &Target,
    kind: ChangeKind,
    content: Option<&str>,
) -> Result<(FileFact, FileDiff), ToolError> {
    match content {
        Some(text) => {
            if let Some(parent) = target.abs.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ToolError::failed(e.to_string()))?;
            }
            std::fs::write(&target.abs, text).map_err(|e| ToolError::failed(e.to_string()))?;
            ctx.ws.mark_read(&target.abs);
        }
        None => {
            std::fs::remove_file(&target.abs).map_err(|e| ToolError::failed(e.to_string()))?;
            ctx.ws.forget(&target.abs);
        }
    }

    Ok(FileFact::of(
        target.rel.clone(),
        kind,
        target.current.as_deref(),
        content,
    ))
}

/// То же плюс инварианты правки: файл должен быть прочитан и не должен был
/// измениться после чтения (§8).
pub(crate) fn look_editable(ctx: &ToolCtx, raw: &str) -> Result<Target, ToolError> {
    let target = look(ctx, raw)?;
    ensure_editable(ctx, &target)?;
    Ok(target)
}

fn ensure_editable(ctx: &ToolCtx, target: &Target) -> Result<(), ToolError> {
    if !ctx.ws.was_read(&target.abs) {
        // Форма из §5.9: `Правка отклонена · src/app.rs не прочитан в этой сессии`.
        return Err(ToolError::rejected(format!(
            "{} не прочитан в этой сессии",
            target.rel
        )));
    }

    if ctx.ws.read_record(&target.abs) != target.fingerprint {
        // Забываем отпечаток: пока файл не перечитан, править его вслепую
        // нельзя, и повторная попытка упрётся в первую же проверку.
        ctx.ws.forget(&target.abs);
        return Err(ToolError::rejected(format!(
            "{} изменился после чтения — перечитай его",
            target.rel
        )));
    }

    Ok(())
}

/// Читает файл с диска. Двоичный и отсутствующий различаются: первый — отказ,
/// второй — законный `None`.
pub(crate) fn read_text(path: &Path) -> Result<Option<String>, ToolError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ToolError::failed(e.to_string())),
    };
    if bytes.iter().take(8_192).any(|b| *b == 0) {
        return Err(ToolError::failed("двоичный файл, править нечего"));
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}
