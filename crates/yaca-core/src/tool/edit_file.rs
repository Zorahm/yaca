//! Point edit by anchor — one or several at a time.
//!
//! Three invariants of §8 meet here: the file must have been read, must not have
//! changed since the read, and the anchor must be unambiguous. The last is not
//! pedantry: "replace the first occurrence" in a file with three
//! identical lines means the model does not know what it is editing.
//!
//! There can be several edits in one call, and that is not a convenience but a fix. The model
//! reads the file once and counts the whole batch against it at once; the tool's description
//! itself tells it to take a wider anchor, "add more surrounding lines". In a densely
//! commented file the surroundings of one edit are the body of the neighbouring one, and
//! fourteen separate calls land on fourteen different versions of the file:
//! the first passes, the second's anchor is gone along with the lines the first
//! rewrote. Within one call all anchors are searched in one text — the one the model
//! counted them against — and overlapping ranges become a named conflict instead of a
//! cryptic "could not find this text".
//!
//! The call is atomic: if one edit does not apply, none is. "Modified
//! file" with half the edits not landed would be a lie about the state (§0),
//! and after a refusal the model has something to re-read and something to retry with.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::files;
use super::{FileChange, Narration, Tool, ToolCtx, ToolOutput};
use crate::change::ChangeKind;
use crate::error::ToolError;
use crate::intent::Intent;
use crate::permission::PermissionNeed;
use crate::workspace::Workspace;

pub struct EditFile;

#[async_trait]
impl Tool for EditFile {
    fn name(&self) -> &'static str {
        "Edit"
    }

    /// English, like `bash`: this is an instruction that has to work, not text
    /// for a human (CLAUDE.md, "Language").
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

    /// Only `path`: beyond it comes either the flat pair or `edits`, and "exactly one of the
    /// two forms" cannot be expressed in the schema. What arrives is parsed by [`Edit::list`].
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

    /// Asked in "Manual" mode (§8). There are no `+12 −3` counters here: they are
    /// known only after the edit, and the point of the question is to stop the agent
    /// before the file is touched.
    fn permission(&self, ws: &Workspace, input: &Value) -> Option<PermissionNeed> {
        let raw = input["path"].as_str()?;
        let abs = ws.resolve_new(raw).ok()?;
        Some(files::edit_need(ws, self.name(), raw, abs))
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let raw = input["path"]
            .as_str()
            .ok_or_else(|| ToolError::failed("path is required"))?;
        let edits = Edit::list(&input)?;

        let target = files::look_editable(ctx, raw)?;
        let Some(base) = target.current.clone() else {
            return Err(ToolError::failed(format!("not found: {raw}")));
        };

        let (updated, replaced) = apply(&base, &edits, &target.rel)?;
        let (fact, diff) = files::write(ctx, &target, ChangeKind::Modified, Some(&updated))?;

        Ok(ToolOutput::new(format!(
            "Modified {} (+{} −{}{})\n",
            target.rel,
            fact.added,
            fact.removed,
            tail(edits.len(), replaced)
        ))
        .changed(FileChange { fact, diff }))
    }
}

/// One edit: what to replace and with what.
struct Edit {
    old: String,
    new: String,
    replace_all: bool,
}

impl Edit {
    /// The edits of a call — from `edits` or from the flat pair.
    ///
    /// Both forms at once is not a convenience but an ambiguity: it is unclear what
    /// to apply and in what order. Refusing here is cheaper than a guess.
    fn list(input: &Value) -> Result<Vec<Edit>, ToolError> {
        let flat = input.get("old_string").is_some() || input.get("new_string").is_some();

        if let Some(items) = input.get("edits").and_then(Value::as_array) {
            if flat {
                return Err(ToolError::failed(
                    "either edits or old_string/new_string — not both at once",
                ));
            }
            if items.is_empty() {
                return Err(ToolError::failed("edits is empty — nothing to edit"));
            }
            return items.iter().map(Edit::one).collect();
        }

        Ok(vec![Edit::one(input)?])
    }

    fn one(value: &Value) -> Result<Edit, ToolError> {
        let old = value["old_string"]
            .as_str()
            .ok_or_else(|| ToolError::failed("old_string is required"))?;
        let new = value["new_string"]
            .as_str()
            .ok_or_else(|| ToolError::failed("new_string is required"))?;

        if old.is_empty() {
            return Err(ToolError::failed(
                "empty old_string — use Write to create a file",
            ));
        }
        if old == new {
            return Err(ToolError::failed("old_string and new_string are identical"));
        }

        Ok(Edit {
            old: old.to_string(),
            new: new.to_string(),
            replace_all: value["replace_all"].as_bool().unwrap_or(false),
        })
    }
}

/// The place one replacement will take, in the coordinates of the original text.
struct Claim {
    at: std::ops::Range<usize>,
    new: String,
    /// Whose replacement this is. A conflict message names it by number.
    edit: usize,
}

/// Applies all edits to one text, returning the new text and the number of replacements.
///
/// All anchors are searched in `base` — the text the model counted them against.
/// Replacements are applied right to left: so early ranges do not shift from
/// later ones, and the order of edits in the list makes no difference.
///
/// The edit number in messages is for humans, counted from one, and only when
/// there is more than one edit: "edit 1 of 1" clarifies nothing.
fn apply(base: &str, edits: &[Edit], rel: &str) -> Result<(String, usize), ToolError> {
    let many = edits.len() > 1;
    let mut claims: Vec<Claim> = Vec::new();

    let crlf = base.contains("\r\n");

    for (index, edit) in edits.iter().enumerate() {
        let mut old = edit.old.as_str();
        let mut new = edit.new.as_str();
        let mut found: Vec<usize> = base.match_indices(old).map(|(at, _)| at).collect();
        // Read shows a CRLF file's lines without the `\r`, so a multi-line anchor
        // arrives with bare `\n` and matches nothing. The same anchor in the
        // file's own line endings is the text the model meant; what replaces it
        // gets them too, so the file does not end up with mixed endings.
        let (crlf_old, crlf_new);
        if found.is_empty() && crlf {
            crlf_old = with_crlf(old);
            crlf_new = with_crlf(new);
            found = base.match_indices(&crlf_old).map(|(at, _)| at).collect();
            if !found.is_empty() {
                old = &crlf_old;
                new = &crlf_new;
            }
        }
        let which = which(many, index);

        match found.len() {
            0 => {
                return Err(ToolError::failed(format!(
                    "could not find this text in {rel}{which}"
                )))
            }
            n if n > 1 && !edit.replace_all => {
                // The anchor uniqueness invariant (§8): §5.9 shows this as a
                // rejection, not a failure — the guard worked, it did not break.
                return Err(ToolError::rejected(format!(
                    "anchor occurs {n} times in {rel}{which} — add more surrounding \
                     lines or set replace_all"
                )));
            }
            _ => {}
        }

        for at in found {
            claims.push(Claim {
                at: at..at + old.len(),
                new: new.to_string(),
                edit: index,
            });
        }
    }

    claims.sort_by_key(|claim| claim.at.start);
    for pair in claims.windows(2) {
        let (left, right) = (&pair[0], &pair[1]);
        if right.at.start < left.at.end {
            // Exactly the case the edits were gathered into one call for: taken
            // separately, the second would silently not find its anchor, because the
            // first ate it. A named conflict is something the model can fix without
            // re-reading the file again.
            return Err(ToolError::rejected(format!(
                "edits {} and {} modify the same chunk of {rel} — \
                 combine them into one",
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

/// Every line break as `\r\n`, whichever it was.
fn with_crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// `, edit 3` — only when there are several edits.
fn which(many: bool, index: usize) -> String {
    if many {
        format!(", edit {}", index + 1)
    } else {
        String::new()
    }
}

/// The tail of the caption: how many edits and how many occurrences were replaced.
///
/// Stays silent when there is nothing to say: one edit, one occurrence — and it is visible anyway.
fn tail(edits: usize, replaced: usize) -> String {
    match (edits > 1, replaced > edits) {
        (false, false) => String::new(),
        (false, true) => format!(", occurrences replaced: {replaced}"),
        (true, false) => format!(", edits: {edits}"),
        (true, true) => format!(", edits: {edits}, occurrences replaced: {replaced}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;

    #[tokio::test]
    async fn edits_by_unambiguous_anchor() {
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
            "edit moves to tree immediately"
        );
    }

    #[tokio::test]
    async fn a_multi_line_anchor_finds_its_text_in_a_crlf_file() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "let x = 1;\r\nlet y = 2;\r\nlet z = 3;\r\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "old_string": "let x = 1;\nlet y = 2;",
                    "new_string": "let x = 1;\nlet w = 0;\nlet y = 2;",
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "let x = 1;\r\nlet w = 0;\r\nlet y = 2;\r\nlet z = 3;\r\n",
            "no bare \\n may end up in a CRLF file"
        );
    }

    #[tokio::test]
    async fn unread_file_cannot_be_edited() {
        let dir = TempDir::new();
        dir.write("a.rs", "body\n");
        let (ctx, _ws) = ctx_for(&dir);

        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "body", "new_string": "carcass" }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
        assert!(err.to_string().contains("was not read in this session"));
    }

    #[tokio::test]
    async fn file_changed_after_reading_must_be_reread() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "one\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&path, "someone else\n").unwrap();

        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "one", "new_string": "two" }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
        assert!(err.to_string().contains("changed since it was read"));
        assert!(
            !ws.was_read(&path),
            "fingerprint forgotten — otherwise retry would succeed"
        );
    }

    #[tokio::test]
    async fn ambiguous_anchor_is_rejected() {
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

        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
        assert!(err.to_string().contains("3 times"));
    }

    #[tokio::test]
    async fn replace_all_lifts_uniqueness_requirement() {
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

        assert!(out.text.contains("occurrences replaced: 2"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "x = 2;\nx = 2;\n");
    }

    #[tokio::test]
    async fn missing_anchor_is_failure_not_rejection() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "body\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        // A model's miss is not a guard that fired: §5.9 colours them differently.
        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "missing", "new_string": "x" }),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }), "got {err:?}");
    }

    #[tokio::test]
    async fn edit_applies_on_top_of_previous_one() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "one\ntwo\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "one", "new_string": "first" }),
            )
            .await
            .unwrap();
        EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "two", "new_string": "second" }),
            )
            .await
            .unwrap();

        // The second edit must see the first, or the agent overwrites itself.
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "first\nsecond\n"
        );
    }

    #[tokio::test]
    async fn multiple_edits_apply_in_one_call() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "one\ntwo\nthree\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let out = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "edits": [
                        { "old_string": "one", "new_string": "first" },
                        { "old_string": "three", "new_string": "third" },
                    ],
                }),
            )
            .await
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "first\ntwo\nthird\n"
        );
        // One call — one action line and one diff.
        assert!(out.text.contains("edits: 2"), "{}", out.text);
        assert!(out.changed.is_some());
    }

    #[tokio::test]
    async fn anchors_matched_against_original_text_not_neighbouring_work() {
        let dir = TempDir::new();
        // Taken separately this does not work: the first edit creates a second
        // occurrence of the second one's anchor, and that one is rejected as ambiguous — though in the file
        // the model read, the anchor was exactly one.
        let path = dir.write("a.rs", "x = 1;\ny = 2;\n");
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
            "x = 3;\nx = 1;\n",
            "each edit landed where it was calculated"
        );
    }

    #[tokio::test]
    async fn overlapping_edits_name_each_other_and_write_nothing() {
        let dir = TempDir::new();
        // Exactly a case from real life: the anchor was taken wider, "add more surrounding lines",
        // and one edit's surroundings turned out to be the body of its neighbour.
        let path = dir.write("a.rs", "one\ntwo\nthree\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "edits": [
                        { "old_string": "one\ntwo", "new_string": "first\nsecond" },
                        { "old_string": "two\nthree", "new_string": "second\nthird" },
                    ],
                }),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ToolError::Rejected(_)), "got {err:?}");
        let text = err.to_string();
        assert!(text.contains("edits 1 and 2"), "{text}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "one\ntwo\nthree\n",
            "call is atomic: if one fails — none apply"
        );
    }

    #[tokio::test]
    async fn miss_in_one_edit_cancels_entire_batch() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "one\ntwo\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "edits": [
                        { "old_string": "one", "new_string": "first" },
                        { "old_string": "missing", "new_string": "x" },
                    ],
                }),
            )
            .await
            .unwrap_err();

        // The miss is named by number: otherwise it is unclear which of ten to fix.
        assert!(err.to_string().contains("edit 2"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\ntwo\n");
    }

    #[tokio::test]
    async fn single_edit_has_no_number() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "body\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({ "path": "a.rs", "old_string": "missing", "new_string": "x" }),
            )
            .await
            .unwrap_err();

        // "edit 1 of 1" clarifies nothing.
        assert!(!err.to_string().contains("edit"), "{err}");
    }

    #[tokio::test]
    async fn both_forms_at_once_is_refusal_not_a_guess() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "body\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(
                &ctx,
                json!({
                    "path": "a.rs",
                    "old_string": "body",
                    "new_string": "carcass",
                    "edits": [{ "old_string": "body", "new_string": "flesh" }],
                }),
            )
            .await
            .unwrap_err();

        assert!(err.to_string().contains("not both at once"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "body\n");
    }

    #[tokio::test]
    async fn empty_edits_list_is_refusal() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "body\n");
        let (ctx, ws) = ctx_for(&dir);
        ws.mark_read(&path);

        let err = EditFile
            .run(&ctx, json!({ "path": "a.rs", "edits": [] }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("nothing to edit"), "{err}");
    }

    #[tokio::test]
    async fn replace_all_inside_batch_counts_all_occurrences() {
        let dir = TempDir::new();
        let path = dir.write("a.rs", "x = 1;\nx = 1;\ny = 2;\n");
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
            "x = 9;\nx = 9;\ny = 8;\n"
        );
        assert!(out.text.contains("occurrences replaced: 3"), "{}", out.text);
    }
}

#[cfg(test)]
mod guard_tests {
    use super::*;
    use crate::permission::{Invariant, PermissionKind};
    use crate::testutil::TempDir;

    #[test]
    fn editing_git_configuration_is_an_invariant_in_every_mode() {
        let dir = TempDir::new();
        dir.write(".git/config", "[core]\n");
        let ws = Workspace::open(dir.path()).unwrap();
        let input = json!({ "path": ".git/config", "edits": [] });
        let need = EditFile.permission(&ws, &input).unwrap();
        assert_eq!(need.invariant, Some(Invariant::GitConfig));
        assert_eq!(need.kind, PermissionKind::EditFile);
    }

    #[test]
    fn editing_an_ordinary_file_is_not_an_invariant() {
        let dir = TempDir::new();
        dir.write("src/a.rs", "fn a() {}\n");
        let ws = Workspace::open(dir.path()).unwrap();
        let need = EditFile
            .permission(&ws, &json!({ "path": "src/a.rs", "edits": [] }))
            .unwrap();
        assert_eq!(need.invariant, None);
    }
}
