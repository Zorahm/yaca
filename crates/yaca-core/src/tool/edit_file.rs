//! Точечная правка по якорю — одна или несколько за раз.
//!
//! Три инварианта §8 сходятся именно здесь: файл должен быть прочитан, не
//! должен был измениться после чтения, а якорь должен быть однозначен.
//! Последнее — не педантизм: «замени первое вхождение» в файле с тремя
//! одинаковыми строками означает, что модель не знает, что именно правит.
//!
//! Правок в вызове может быть несколько, и это не удобство, а починка. Модель
//! читает файл один раз и считает по нему всю пачку сразу; описание тула само
//! велит ей брать якорь пошире, «добавь окружающих строк». В плотно
//! закомментированном файле окружение одной правки — это тело соседней, и
//! четырнадцать отдельных вызовов ложатся на четырнадцать разных версий файла:
//! первая проходит, у второй якорь исчез вместе со строками, которые первая
//! переписала. Внутри одного вызова все якоря ищутся в одном тексте — том
//! самом, по которому модель их считала, — а пересечения диапазонов становятся
//! названным конфликтом вместо загадочного «не нашёл этот текст».
//!
//! Вызов атомарен: не сложилась одна правка — не применяется ни одна. «Изменён
//! файл» при половине непроехавших правок было бы враньём про состояние (§0),
//! а модели после отказа есть что перечитать и чем повторить.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files;
use super::{FileChange, Narration, Tool, ToolCtx, ToolOutput};
use crate::change::ChangeKind;
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::{PermissionKind, PermissionNeed};
use crate::workspace::Workspace;

pub struct EditFile;

#[async_trait]
impl Tool for EditFile {
    fn name(&self) -> &'static str {
        "Edit"
    }

    /// Английское, как у `bash`: это инструкция, которая обязана сработать,
    /// а не текст для человека (CLAUDE.md, «Язык»).
    fn description(&self) -> &'static str {
        "Replace text in a file. `old_string` must occur exactly once — widen it with \
         surrounding lines until it does. Read the file before editing it.\n\
         Several changes to the same file go in ONE call, listed in `edits` — never one \
         call per change. Within a call every anchor is matched against the file as you \
         read it, so your edits cannot shift each other out from under themselves, and \
         two that overlap are reported by number instead of silently missing. The call \
         is all-or-nothing: if one edit does not apply, none of them do."
    }

    fn properties(&self) -> Value {
        let old = json!({
            "type": "string",
            "description": "Text to replace, verbatim and with its indentation.",
        });
        let new = json!({
            "type": "string",
            "description": "What to put in its place. An empty string deletes the text.",
        });
        let replace_all = json!({
            "type": "boolean",
            "description": "Replace every occurrence. Without it exactly one is required.",
        });
        json!({
            "path": { "type": "string", "description": "Path relative to the project root." },
            "old_string": old,
            "new_string": new,
            "replace_all": replace_all,
            "edits": {
                "type": "array",
                "description": "Several changes to this file at once, applied in one go. \
                    Use this instead of `old_string`/`new_string` whenever there is more \
                    than one change, and instead of several calls on the same file.",
                "items": {
                    "type": "object",
                    "properties": {
                        "old_string": old,
                        "new_string": new,
                        "replace_all": replace_all,
                    },
                    "required": ["old_string", "new_string"],
                },
            },
        })
    }

    /// Только `path`: дальше либо плоская пара, либо `edits`, и «ровно одна из
    /// двух форм» схемой не выражается. Что пришло, разбирает [`Edit::list`].
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

    /// Спрашивается в «Ручном» (§8). Счётчиков `+12 −3` здесь нет: они
    /// известны только после правки, а смысл запроса — остановить агента
    /// до того, как файл будет тронут.
    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let raw = input["path"].as_str()?;
        let abs = ws.resolve_new(raw).ok()?;
        let name = abs
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| raw.to_string());
        Some(
            PermissionNeed::new(PermissionKind::EditFile, self.name(), name)
                .detail(ws.display(&abs))
                .path(abs),
        )
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан path"))?;
        let edits = Edit::list(&input)?;

        let target = files::look_editable(ctx, raw)?;
        let Some(base) = target.current.clone() else {
            return Err(ToolError::failed(format!("не найдено: {raw}")));
        };

        let (updated, replaced) = apply(&base, &edits, &target.rel)?;
        let (fact, diff) = files::write(ctx, &target, ChangeKind::Modified, Some(&updated))?;

        Ok(ToolOutput::new(format!(
            "Изменён {} (+{} −{}{})\n",
            target.rel,
            fact.added,
            fact.removed,
            tail(edits.len(), replaced)
        ))
        .changed(FileChange { fact, diff }))
    }
}

/// Одна правка: что заменить и чем.
struct Edit {
    old: String,
    new: String,
    replace_all: bool,
}

impl Edit {
    /// Правки вызова — из `edits` или из плоской пары.
    ///
    /// Обе формы разом — не удобство, а неоднозначность: непонятно, что
    /// применять и в каком порядке. Отказ здесь дешевле догадки.
    fn list(input: &Value) -> Result<Vec<Edit>, ToolError> {
        let flat = input.get("old_string").is_some() || input.get("new_string").is_some();

        if let Some(items) = input.get("edits").and_then(Value::as_array) {
            if flat {
                return Err(ToolError::failed(
                    "или edits, или old_string/new_string — не оба сразу",
                ));
            }
            if items.is_empty() {
                return Err(ToolError::failed("edits пуст — править нечего"));
            }
            return items.iter().map(Edit::one).collect();
        }

        Ok(vec![Edit::one(input)?])
    }

    fn one(value: &Value) -> Result<Edit, ToolError> {
        let old = value["old_string"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан old_string"))?;
        let new = value["new_string"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан new_string"))?;

        if old.is_empty() {
            return Err(ToolError::failed(
                "пустой old_string — для создания файла есть Write",
            ));
        }
        if old == new {
            return Err(ToolError::failed("old_string и new_string совпадают"));
        }

        Ok(Edit {
            old: old.to_string(),
            new: new.to_string(),
            replace_all: value["replace_all"].as_bool().unwrap_or(false),
        })
    }
}

/// Место, которое займёт одна замена, — в координатах исходного текста.
struct Claim {
    at: std::ops::Range<usize>,
    new: String,
    /// Чья это замена. В сообщении о конфликте называется по номеру.
    edit: usize,
}

/// Применяет все правки к одному тексту, отдавая новый текст и число замен.
///
/// Все якоря ищутся в `base` — том тексте, по которому модель их и считала.
/// Применяются замены справа налево: так ранние диапазоны не съезжают от
/// поздних, и порядок правок в списке ни на что не влияет.
///
/// Номер правки в сообщениях — человеческий, с единицы, и только когда правок
/// больше одной: «правка 1 из 1» ничего не уточняет.
fn apply(base: &str, edits: &[Edit], rel: &str) -> Result<(String, usize), ToolError> {
    let many = edits.len() > 1;
    let mut claims: Vec<Claim> = Vec::new();

    for (index, edit) in edits.iter().enumerate() {
        let found: Vec<usize> = base.match_indices(&edit.old).map(|(at, _)| at).collect();
        let which = which(many, index);

        match found.len() {
            0 => {
                return Err(ToolError::failed(format!(
                    "не нашёл этот текст в {rel}{which}"
                )))
            }
            n if n > 1 && !edit.replace_all => {
                // Инвариант уникальности якоря (§8): §5.9 покажет это
                // отклонением, а не сбоем, — защита сработала, а не сломалась.
                return Err(ToolError::rejected(format!(
                    "якорь встречается {n} раз в {rel}{which} — добавь окружающих \
                     строк или поставь replace_all"
                )));
            }
            _ => {}
        }

        for at in found {
            claims.push(Claim {
                at: at..at + edit.old.len(),
                new: edit.new.clone(),
                edit: index,
            });
        }
    }

    claims.sort_by_key(|claim| claim.at.start);
    for pair in claims.windows(2) {
        let (left, right) = (&pair[0], &pair[1]);
        if right.at.start < left.at.end {
            // Ровно тот случай, ради которого правки собраны в один вызов:
            // порознь вторая молча не нашла бы своего якоря, потому что его
            // съела первая. Названный конфликт — это то, что модель может
            // починить, не перечитывая файл заново.
            return Err(ToolError::rejected(format!(
                "правки {} и {} меняют один и тот же кусок {rel} — \
                 объедини их в одну",
                left.edit + 1,
                right.edit + 1
            )));
        }
    }

    let mut out = base.to_string();
    for claim in claims.iter().rev() {
        out.replace_range(claim.at.clone(), &claim.new);
    }
    Ok((out, claims.len()))
}

/// `, правка 3` — только когда правок несколько.
fn which(many: bool, index: usize) -> String {
    if many {
        format!(", правка {}", index + 1)
    } else {
        String::new()
    }
}

/// Хвост подписи: сколько правок и сколько вхождений заменено.
///
/// Молчит, когда сказать нечего: одна правка, одно вхождение — и так видно.
fn tail(edits: usize, replaced: usize) -> String {
    match (edits > 1, replaced > edits) {
        (false, false) => String::new(),
        (false, true) => format!(", заменено вхождений: {replaced}"),
        (true, false) => format!(", правок: {edits}"),
        (true, true) => format!(", правок: {edits}, заменено вхождений: {replaced}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn правит_по_однозначному_якорю() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "let x = 1;\nlet y = 2;\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let out = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "let y = 2;", "new_string": "let y = 42;" }),
            )
            .await
            .unwrap();

        assert_eq!(out.changed.unwrap().fact.added, 1);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "let x = 1;\nlet y = 42;\n",
            "правка уезжает в дерево сразу"
        );
    }

    #[tokio::test]
    async fn непрочитанный_файл_править_нельзя() {
        let dir = TempDir::new();
        dir.write("a.rs", "тело\n");
        let (ctx, _ws) = ctx_for(&dir);

        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "тело", "new_string": "туша" }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "получили {err:?}");
        assert!(err.to_string().contains("не прочитан в этой сессии"));
    }

    #[tokio::test]
    async fn изменившийся_после_чтения_файл_надо_перечитать() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "раз\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&path, "кто-то другой\n").unwrap();

        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "раз", "new_string": "два" }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "получили {err:?}");
        assert!(err.to_string().contains("изменился после чтения"));
        assert!(
            !ws.was_read(&path),
            "отпечаток забыт — иначе повтор пройдёт"
        );
    }

    #[tokio::test]
    async fn неоднозначный_якорь_отклоняется() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "x = 1;\nx = 1;\nx = 1;\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "x = 1;", "new_string": "x = 2;" }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "получили {err:?}");
        assert!(err.to_string().contains("3 раз"));
    }

    #[tokio::test]
    async fn replace_all_снимает_требование_однозначности() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "x = 1;\nx = 1;\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let out = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "old_string": "x = 1;",
                    "new_string": "x = 2;",
                    "replace_all": true
                }),
            )
            .await
            .unwrap();

        assert!(out.text.contains("заменено вхождений: 2"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "x = 2;\nx = 2;\n");
    }

    #[tokio::test]
    async fn ненайденный_якорь_это_сбой_а_не_отклонение() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "тело\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        // Промах модели — не сработавшая защита: §5.9 красит их по-разному.
        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "нету", "new_string": "x" }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }), "получили {err:?}");
    }

    #[tokio::test]
    async fn правка_ложится_поверх_предыдущей() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "раз\nдва\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "раз", "new_string": "один" }),
            )
            .await
            .unwrap();
        EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "два", "new_string": "второй" }),
            )
            .await
            .unwrap();

        // Вторая правка обязана видеть первую, иначе агент затрёт сам себя.
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "один\nвторой\n"
        );
    }
    #[tokio::test]
    async fn несколько_правок_ложатся_одним_вызовом() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "раз
два
три
");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let out = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "edits": [
                        { "old_string": "раз", "new_string": "one" },
                        { "old_string": "три", "new_string": "three" },
                    ],
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "one
два
three
"
        );
        // Один вызов — одна строка действия и один дифф.
        assert!(out.text.contains("правок: 2"), "{}", out.text);
        assert!(out.changed.is_some());
    }

    #[tokio::test]
    async fn якоря_ищутся_в_исходном_тексте_а_не_в_работе_соседа() {
        let dir = TempDir::new();
        // Порознь это не работает: первая правка создаёт второе вхождение
        // якоря второй, и та отклоняется как неоднозначная — хотя в файле,
        // который модель читала, якорь был ровно один.
        let path = dir.write("a.rs", "x = 1;
y = 2;
");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "edits": [
                        { "old_string": "y = 2;", "new_string": "x = 1;" },
                        { "old_string": "x = 1;", "new_string": "x = 3;" },
                    ],
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "x = 3;
x = 1;
",
            "каждая правка попала туда, куда её посчитали"
        );
    }

    #[tokio::test]
    async fn пересекающиеся_правки_называют_друг_друга_и_ничего_не_пишут() {
        let dir = TempDir::new();
        // Ровно случай из жизни: якорь взят пошире, «добавь окружающих строк»,
        // и окружение одной правки оказалось телом соседней.
        let path = dir.write("a.rs", "раз
два
три
");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "edits": [
                        { "old_string": "раз
два", "new_string": "one
two" },
                        { "old_string": "два
три", "new_string": "two
three" },
                    ],
                }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "получили {err:?}");
        let text = err.to_string();
        assert!(text.contains("правки 1 и 2"), "{text}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "раз
два
три
",
            "вызов атомарен: не сложилась одна — не применилась ни одна"
        );
    }

    #[tokio::test]
    async fn промах_одной_правки_отменяет_всю_пачку() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "раз
два
");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "edits": [
                        { "old_string": "раз", "new_string": "one" },
                        { "old_string": "нету", "new_string": "x" },
                    ],
                }),
            )
            .await
            .unwrap_err();

        // Промах назван номером: иначе непонятно, какую из десяти чинить.
        assert!(err.to_string().contains("правка 2"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "раз
два
");
    }

    #[tokio::test]
    async fn у_одиночной_правки_номера_нет() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "тело
");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "нету", "new_string": "x" }),
            )
            .await
            .unwrap_err();

        // «правка 1 из 1» ничего не уточняет.
        assert!(!err.to_string().contains("правка"), "{err}");
    }

    #[tokio::test]
    async fn две_формы_разом_это_отказ_а_не_догадка() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "тело
");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "old_string": "тело",
                    "new_string": "туша",
                    "edits": [{ "old_string": "тело", "new_string": "плоть" }],
                }),
            )
            .await
            .unwrap_err();

        assert!(err.to_string().contains("не оба сразу"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "тело
");
    }

    #[tokio::test]
    async fn пустой_список_правок_это_отказ() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "тело
");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(&ctx, json!({ "path": "a.rs", "edits": [] }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("править нечего"), "{err}");
    }

    #[tokio::test]
    async fn replace_all_внутри_пачки_считает_все_вхождения() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "x = 1;
x = 1;
y = 2;
");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let out = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "edits": [
                        { "old_string": "x = 1;", "new_string": "x = 9;", "replace_all": true },
                        { "old_string": "y = 2;", "new_string": "y = 8;" },
                    ],
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "x = 9;
x = 9;
y = 8;
"
        );
        assert!(out.text.contains("заменено вхождений: 3"), "{}", out.text);
    }

}
