//! Построение диффа (§6.3).
//!
//! Считается в ядре, а не в вебвью, по трём причинам: результат одинаков в
//! ленте и в панели, он попадает в поток событий как данные, и §12 требует
//! 60 fps во время стриминга — разбирать текст в вебвью параллельно дельтам
//! не выйдет.

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, InlineChange, TextDiff};
use ts_rs::TS;

/// Строк контекста вокруг изменения. Три — то, что показывают все,
/// и то, чего хватает, чтобы узнать место.
const CONTEXT: usize = 3;

/// §12: дифф длиннее двух тысяч строк рендерится окном. Дальше этого
/// предела ядру нечего отдавать — интерфейс всё равно покажет часть.
const MAX_LINES: usize = 2_000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub hunks: Vec<Hunk>,
    pub added: u32,
    pub removed: u32,
    /// Дифф обрезан по [`MAX_LINES`].
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

impl Hunk {
    /// `@@ -12,7 +12,9 @@` — заголовок ханка (§6.3).
    pub fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old_start, self.old_lines, self.new_start, self.new_lines
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    /// Номер строки до правки, считая от 1. Жёлоб слева (§6.3).
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
    /// Что именно изменилось внутри строки — пословная подсветка (§6.3).
    /// Диапазоны в символах, не в байтах: интерфейс режет строку по символам.
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum LineKind {
    Context,
    Add,
    Del,
}

pub fn compute(before: &str, after: &str) -> FileDiff {
    let diff = TextDiff::from_lines(before, after);

    let mut out = FileDiff::default();

    // Счётчики считаются по всем операциям, а не по отрисованным строкам:
    // `+597 −16` в полосе (§6.1) — про весь файл, даже если показан кусок.
    for op in diff.ops() {
        use similar::DiffTag;
        match op.tag() {
            DiffTag::Delete => out.removed += op.old_range().len() as u32,
            DiffTag::Insert => out.added += op.new_range().len() as u32,
            DiffTag::Replace => {
                out.removed += op.old_range().len() as u32;
                out.added += op.new_range().len() as u32;
            }
            DiffTag::Equal => {}
        }
    }

    let mut emitted = 0usize;

    'groups: for group in diff.grouped_ops(CONTEXT) {
        let Some(first) = group.first() else { continue };
        let old_start = first.old_range().start as u32 + 1;
        let new_start = first.new_range().start as u32 + 1;

        let mut lines: Vec<DiffLine> = Vec::new();
        let mut оборван = false;

        for op in &group {
            for change in diff.iter_inline_changes(op) {
                // Предел проверяется на каждой строке, а не на каждом ханке.
                // Иначе один ханк на пять тысяч строк проезжает мимо лимита:
                // подряд идущие изменения группируются в одну группу.
                if emitted >= MAX_LINES {
                    оборван = true;
                    break;
                }

                let kind = match change.tag() {
                    ChangeTag::Equal => LineKind::Context,
                    ChangeTag::Delete => LineKind::Del,
                    ChangeTag::Insert => LineKind::Add,
                };
                let (text, spans) = inline(&change);
                lines.push(DiffLine {
                    kind,
                    old_line: change.old_index().map(|i| i as u32 + 1),
                    new_line: change.new_index().map(|i| i as u32 + 1),
                    text,
                    spans,
                });
                emitted += 1;
            }
            if оборван {
                break;
            }
        }

        // Длины считаются по выданным строкам: заголовок ханка должен
        // описывать то, что в нём лежит, а не то, что могло бы лежать.
        push_hunk(&mut out, old_start, new_start, lines);

        if оборван {
            out.truncated = true;
            break 'groups;
        }
    }

    out
}

fn push_hunk(out: &mut FileDiff, old_start: u32, new_start: u32, lines: Vec<DiffLine>) {
    if lines.is_empty() {
        return;
    }
    let old_lines = lines.iter().filter(|l| l.old_line.is_some()).count() as u32;
    let new_lines = lines.iter().filter(|l| l.new_line.is_some()).count() as u32;
    out.hunks.push(Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines,
    });
}

/// Собирает текст строки и диапазоны изменившихся кусков.
pub(super) fn inline(change: &InlineChange<'_, str>) -> (String, Vec<Span>) {
    let mut text = String::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut chars = 0u32;

    for (emphasized, value) in change.iter_strings_lossy() {
        let start = chars;
        text.push_str(&value);
        chars += value.chars().count() as u32;
        if emphasized && chars > start {
            // Соседние подсвеченные куски склеиваем: интерфейсу проще
            // отрисовать один диапазон, чем цепочку из трёх букв.
            match spans.last_mut() {
                Some(prev) if prev.end == start => prev.end = chars,
                _ => spans.push(Span { start, end: chars }),
            }
        }
    }

    // Перевод строки в дифф не едет: жёлоб и фон рисуются по строке целиком.
    let trimmed = text.trim_end_matches(['\n', '\r']);
    let limit = trimmed.chars().count() as u32;
    text.truncate(trimmed.len());

    spans.retain(|s| s.start < limit);
    for s in &mut spans {
        s.end = s.end.min(limit);
    }

    (text, spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn считает_добавленное_и_удалённое() {
        let d = compute("раз\nдва\nтри\n", "раз\nдва с половиной\nтри\nчетыре\n");
        assert_eq!((d.added, d.removed), (2, 1));
        assert!(!d.truncated);
    }

    #[test]
    fn одинаковые_файлы_дают_пустой_дифф() {
        let d = compute("одно и то же\n", "одно и то же\n");
        assert!(d.hunks.is_empty());
        assert_eq!((d.added, d.removed), (0, 0));
    }

    #[test]
    fn номера_строк_считаются_от_единицы() {
        let d = compute("a\nb\nc\n", "a\nB\nc\n");
        let lines = &d.hunks[0].lines;

        let del = lines.iter().find(|l| l.kind == LineKind::Del).unwrap();
        assert_eq!(del.old_line, Some(2));
        assert_eq!(del.new_line, None);
        assert_eq!(del.text, "b");

        let add = lines.iter().find(|l| l.kind == LineKind::Add).unwrap();
        assert_eq!(add.new_line, Some(2));
        assert_eq!(add.old_line, None);

        let ctx = lines.iter().find(|l| l.kind == LineKind::Context).unwrap();
        assert_eq!((ctx.old_line, ctx.new_line), (Some(1), Some(1)));
    }

    #[test]
    fn перевод_строки_не_едет_в_текст() {
        let d = compute("a\n", "b\n");
        for line in &d.hunks[0].lines {
            assert!(
                !line.text.contains('\n'),
                "в тексте строки перевод: {line:?}"
            );
        }
    }

    #[test]
    fn пословная_подсветка_отмечает_только_изменившееся() {
        let d = compute("let x = 1;\n", "let x = 42;\n");
        let add = d.hunks[0]
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Add)
            .unwrap();

        assert!(!add.spans.is_empty(), "подсветка должна быть");
        // Диапазоны обязаны лежать внутри строки, иначе интерфейс порежет
        // её не там и покажет мусор.
        let len = add.text.chars().count() as u32;
        for s in &add.spans {
            assert!(s.start < s.end && s.end <= len, "{s:?} вне строки {len}");
        }

        let подсвечено: String = add
            .spans
            .iter()
            .flat_map(|s| {
                add.text
                    .chars()
                    .skip(s.start as usize)
                    .take((s.end - s.start) as usize)
            })
            .collect();
        assert!(подсвечено.contains("42"), "подсвечено: {подсвечено:?}");
        assert!(!подсвечено.contains("let"), "контекст подсвечивать нечего");
    }

    #[test]
    fn кириллица_не_ломает_диапазоны() {
        let d = compute("привет мир\n", "привет город\n");
        let add = d.hunks[0]
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Add)
            .unwrap();
        let len = add.text.chars().count() as u32;
        for s in &add.spans {
            assert!(s.end <= len, "{s:?} вне строки длиной {len}");
        }
    }

    #[test]
    fn создание_файла_это_один_ханк_из_добавлений() {
        let d = compute("", "первая\nвторая\n");
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(d.added, 2);
        assert_eq!(d.removed, 0);
        assert!(d.hunks[0].lines.iter().all(|l| l.kind == LineKind::Add));
    }

    #[test]
    fn огромный_дифф_обрезается_а_не_вываливается_целиком() {
        let before: String = (0..5_000).map(|i| format!("строка {i}\n")).collect();
        let after: String = (0..5_000).map(|i| format!("СТРОКА {i}\n")).collect();
        let d = compute(&before, &after);
        assert!(d.truncated);
        let всего: usize = d.hunks.iter().map(|h| h.lines.len()).sum();
        assert_eq!(всего, MAX_LINES, "выдано {всего} строк");
        // Счётчики полосы — про весь файл, а не про показанный кусок.
        assert_eq!((d.added, d.removed), (5_000, 5_000));
    }

    #[test]
    fn заголовок_ханка_читается_как_дифф() {
        let d = compute("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!(d.hunks[0].header(), "@@ -1,3 +1,3 @@");
    }
}
