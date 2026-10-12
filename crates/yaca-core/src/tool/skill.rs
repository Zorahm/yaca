//! Skills: instructions the project keeps for one kind of task, and the `Skill`
//! tool that hands one over.
//!
//! A skill is `.yaca/skills/<name>/SKILL.md`: a header with `name` and
//! `description`, then a body. The human writes it once, and the model pulls it
//! in when a task calls for it, instead of the human pasting the same
//! instructions into every conversation. Two halves follow from that:
//!
//! * The list — name and one line of description per skill — goes into the
//!   system prompt when the session starts, as the memory index does. Read once
//!   for the same reason: the prompt is the cached prefix of every request.
//! * The tool returns the body. It reads the file at the moment of the call, so
//!   a skill written during the session is usable at once; only the list waits
//!   for the next session.
//!
//! The directory name is the skill's name, and it is the one place a name can
//! come from. A `name` in the header that disagrees with the directory leaves
//! the skill out rather than listing it under a name the tool does not answer
//! to. Names are lowercase letters, digits and hyphens, so no name can express a
//! path, and the workspace boundary is checked on top of that.
//!
//! Read-only, so offered in Plan as well as Agent and asked nothing: a skill is
//! text the human wrote, and reading it changes nothing.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::slash;
use crate::workspace::Workspace;

use super::memory::store::head;

/// Under the project's `.yaca/`, next to commands and memory.
pub const DIR: &str = "skills";
pub const FILE: &str = "SKILL.md";

/// The list goes into every request, so it is capped. Enough for a few dozen
/// one-line descriptions; past that the list says it was cut and the tool's
/// error names the rest.
pub const MAX_LIST_BYTES: usize = 4 * 1024;
/// A description longer than this is cut in the list. The whole of it is still
/// what the skill's own file says.
const MAX_DESCRIPTION_CHARS: usize = 200;
/// The most of a skill file that is read to find its header. A header longer
/// than this is read as absent, which costs the description and nothing else.
const MAX_HEADER_BYTES: usize = 8 * 1024;
/// The most of a skill file the tool hands over: the file as a whole, header
/// included, read up to here.
const MAX_FILE_BYTES: usize = 64 * 1024;
/// What the model reads of the body. A skill is a set of instructions, not a
/// document; past this it is a document, and the model should read it in parts.
const MAX_BODY_BYTES: usize = 32 * 1024;
const MAX_NAME_CHARS: usize = 64;

/// A skill as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub description: String,
}

impl Entry {
    /// The line the list shows for this skill, description after a dash.
    pub fn line(&self) -> String {
        if self.description.is_empty() {
            format!("- {}", self.name)
        } else {
            format!("- {} — {}", self.name, self.description)
        }
    }
}

/// What the system prompt carries: the skills that fit in the cap, in name
/// order, and whether some were left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub entries: Vec<Entry>,
    pub truncated: bool,
}

/// A name a skill may take: lowercase letters, digits and hyphens, and short
/// enough to be read in a list. Nothing that can express `..`, a separator or
/// an absolute path gets through.
pub fn is_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The project-relative path of a skill's file, in the form the workspace
/// checks take.
fn file_rel(name: &str) -> String {
    format!("{}/{DIR}/{name}/{FILE}", crate::project::DIR)
}

/// Every usable skill of the project, in name order. Empty when there is no
/// `.yaca/skills/` at all — a project that never used the feature gets the
/// prompt it always had.
pub fn scan(ws: &Workspace) -> Vec<Entry> {
    let dir = format!("{}/{DIR}", crate::project::DIR);
    // Through the boundary like everything else: a `skills` that is a link out
    // of the project lists nothing.
    let Ok(dir) = ws.resolve_existing(&dir) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut out: Vec<Entry> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !is_skill_name(&name) || !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let (text, _) = read(ws, &name, MAX_HEADER_BYTES)?;
            let (declared, description) = header(&text);
            if declared.as_deref().is_some_and(|d| d != name) {
                return None;
            }
            Some(Entry { name, description })
        })
        .collect();

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The list for the prompt, or `None` when the project has no skills.
pub fn listing(ws: &Workspace) -> Option<Listing> {
    let mut entries = Vec::new();
    let mut bytes = 0;
    let mut truncated = false;
    for entry in scan(ws) {
        let line_len = entry.line().len() + 1;
        if bytes + line_len > MAX_LIST_BYTES {
            truncated = true;
            break;
        }
        bytes += line_len;
        entries.push(entry);
    }
    (!entries.is_empty()).then_some(Listing { entries, truncated })
}

/// The header's `name` (if any) and one-line `description`. The description is
/// cut to a sensible length here, once, so the list and the prompt agree.
fn header(text: &str) -> (Option<String>, String) {
    let Some((block, _)) = slash::split_frontmatter(text) else {
        return (None, String::new());
    };
    let declared = slash::header_value(block, "name").filter(|n| !n.is_empty());
    let description = slash::header_value(block, "description")
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let description = if description.chars().count() > MAX_DESCRIPTION_CHARS {
        let cut: String = description
            .chars()
            .take(MAX_DESCRIPTION_CHARS - 1)
            .collect();
        format!("{}…", cut.trim_end())
    } else {
        description
    };
    (declared, description)
}

/// The text of a skill's file, up to `cap` bytes, and whether it was longer.
/// `None` when the file is not there or is outside the project.
fn read(ws: &Workspace, name: &str, cap: usize) -> Option<(String, bool)> {
    let path = ws.resolve_existing(&file_rel(name)).ok()?;
    head(&path, cap)
}

/// The body of a skill: what follows its header, trimmed.
fn body_of(text: &str) -> &str {
    match slash::split_frontmatter(text) {
        Some((_, body)) => body.trim(),
        None => text.trim(),
    }
}

/// Cuts a body to the byte budget at a character boundary, saying so.
fn capped(body: &str) -> (String, bool) {
    if body.len() <= MAX_BODY_BYTES {
        return (body.to_string(), false);
    }
    let mut cut = MAX_BODY_BYTES;
    while !body.is_char_boundary(cut) {
        cut -= 1;
    }
    let text = format!(
        "{}\n\n[The skill is longer than {MAX_BODY_BYTES} bytes and is cut here. Its file has the rest.]",
        body[..cut].trim_end()
    );
    (text, true)
}

/// The names of every skill, for a failure that has to say what exists.
fn names_for_model(ws: &Workspace) -> String {
    let names: Vec<String> = scan(ws).into_iter().map(|e| e.name).collect();
    if names.is_empty() {
        "The project has no skills.".to_string()
    } else {
        format!("Skills that exist: {}.", names.join(", "))
    }
}

pub struct Skill;

#[async_trait]
impl Tool for Skill {
    fn name(&self) -> &'static str {
        "Skill"
    }

    fn description(&self) -> &'static str {
        "Read a skill: instructions the project keeps for one kind of task, listed by \
         name in the project's skills list. When a task matches a listed skill, call this \
         before you start the task and follow what it returns. Returns the skill's text. \
         A name no skill has fails and says which names exist."
    }

    fn properties(&self) -> Value {
        json!({
            "name": {
                "type": "string",
                "pattern": "^[a-z0-9-]+$",
                "description": "The skill's name, as the list gives it.",
            },
        })
    }

    fn required(&self) -> &'static [&'static str] {
        &["name"]
    }

    fn narration(&self) -> Narration {
        // The interface cannot tell from the call what the skill was for, so
        // the model says it in its own words, as for Memory.
        Narration::Model
    }

    fn intents(&self) -> &'static [Intent] {
        // Read-only, so it belongs to planning as much as to doing.
        &[Intent::Plan, Intent::Agent]
    }

    fn concurrent(&self) -> bool {
        true
    }

    fn target(&self, input: &Value) -> Option<String> {
        input["name"]
            .as_str()
            .filter(|name| is_skill_name(name))
            .map(file_rel)
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let name = input["name"]
            .as_str()
            .ok_or_else(|| ToolError::failed("не задан name"))?;

        // Checked before any path is built from the name. A name that is not
        // one of ours is refused as an invariant, the same as a memory note's.
        if !is_skill_name(name) {
            return Err(ToolError::rejected(format!(
                "{name} — недопустимое имя навыка: строчные латинские буквы, цифры и дефис"
            )));
        }

        let Some((text, cut)) = read(&ctx.ws, name, MAX_FILE_BYTES) else {
            return Err(ToolError::failed_with(
                format!("нет навыка {name}"),
                format!(
                    "No skill named `{name}`. {} Call Skill with one of those names, or \
                     carry on without a skill if none of them fits the task.",
                    names_for_model(&ctx.ws)
                ),
            ));
        };

        let (body, capped_body) = capped(body_of(&text));
        let body = if body.is_empty() {
            "(The skill's file has no text after its header.)".to_string()
        } else {
            body
        };
        let mut out = ToolOutput::new(body);
        if cut || capped_body {
            out = out.truncated(true);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::tool::tests_support::ctx_for;
    use crate::tool::ToolRegistry;

    fn skill(dir: &TempDir, name: &str, text: &str) {
        dir.write(&format!(".yaca/skills/{name}/SKILL.md"), text);
    }

    fn release() -> &'static str {
        "---\nname: release\ndescription: Выпустить новую версию\n---\n\nТег, затем сборка.\n"
    }

    fn ws(dir: &TempDir) -> Workspace {
        Workspace::open(dir.path()).unwrap()
    }

    #[test]
    fn a_project_without_skills_has_no_listing() {
        let dir = TempDir::new();
        assert_eq!(listing(&ws(&dir)), None);
    }

    #[test]
    fn the_listing_is_in_name_order_and_the_same_every_time() {
        let dir = TempDir::new();
        skill(&dir, "release", release());
        skill(
            &dir,
            "audit",
            "---\ndescription: Аудит зависимостей\n---\nтекст",
        );
        skill(&dir, "docs", "без заголовка");

        let first = listing(&ws(&dir)).unwrap();
        let names: Vec<_> = first.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["audit", "docs", "release"]);
        assert!(!first.truncated);
        // The prompt is the cached prefix: the same files must give the same list.
        assert_eq!(listing(&ws(&dir)).unwrap(), first);
    }

    #[test]
    fn a_skill_without_a_header_is_listed_by_name_alone() {
        let dir = TempDir::new();
        skill(&dir, "docs", "просто текст");
        let list = listing(&ws(&dir)).unwrap();
        assert_eq!(list.entries[0].description, "");
        assert_eq!(list.entries[0].line(), "- docs");
    }

    #[test]
    fn a_header_name_that_disagrees_with_the_directory_leaves_the_skill_out() {
        let dir = TempDir::new();
        skill(
            &dir,
            "release",
            "---\nname: relase\ndescription: опечатка\n---\nтекст",
        );
        assert_eq!(listing(&ws(&dir)), None);
    }

    #[test]
    fn directories_that_are_not_skill_names_are_skipped() {
        let dir = TempDir::new();
        skill(&dir, "Upper", "текст");
        skill(&dir, "with_underscore", "текст");
        skill(&dir, "fine-one", "текст");
        let names: Vec<_> = scan(&ws(&dir)).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["fine-one"]);
    }

    #[test]
    fn the_listing_is_capped_and_says_so() {
        let dir = TempDir::new();
        let long = "о".repeat(300);
        for n in 0..60 {
            skill(
                &dir,
                &format!("skill-{n:02}"),
                &format!("---\ndescription: {long}\n---\nтекст"),
            );
        }
        let list = listing(&ws(&dir)).unwrap();
        assert!(list.truncated);
        assert!(list.entries.len() < 60);
        let bytes: usize = list.entries.iter().map(|e| e.line().len() + 1).sum();
        assert!(bytes <= MAX_LIST_BYTES, "{bytes}");
        // Name order means the cut falls at the end, and the first ones stay.
        assert_eq!(list.entries[0].name, "skill-00");
    }

    #[test]
    fn a_long_description_is_cut_in_the_list() {
        let dir = TempDir::new();
        let long = "а".repeat(500);
        skill(&dir, "x", &format!("---\ndescription: {long}\n---\n"));
        let desc = &listing(&ws(&dir)).unwrap().entries[0].description;
        assert!(
            desc.chars().count() <= MAX_DESCRIPTION_CHARS,
            "{}",
            desc.len()
        );
        assert!(desc.ends_with('…'));
    }

    #[tokio::test]
    async fn the_tool_returns_the_body_without_the_header() {
        let dir = TempDir::new();
        skill(&dir, "release", release());
        let (ctx, _ws) = ctx_for(&dir);

        let out = Skill.run(&ctx, json!({ "name": "release" })).await.unwrap();
        assert_eq!(out.text, "Тег, затем сборка.");
        assert!(!out.meta.truncated);
    }

    #[tokio::test]
    async fn an_unknown_name_fails_and_names_the_skills_that_exist() {
        let dir = TempDir::new();
        skill(&dir, "release", release());
        skill(&dir, "audit", "---\ndescription: аудит\n---\nтекст");
        let (ctx, _ws) = ctx_for(&dir);

        let err = Skill
            .run(&ctx, json!({ "name": "deploy" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }), "{err:?}");
        let model = err.for_model();
        assert!(model.contains("audit, release"), "{model}");
    }

    #[tokio::test]
    async fn an_unknown_name_in_a_project_without_skills_says_there_are_none() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        let err = Skill
            .run(&ctx, json!({ "name": "deploy" }))
            .await
            .unwrap_err();
        assert!(err.for_model().contains("no skills"), "{err}");
    }

    #[tokio::test]
    async fn a_name_that_is_not_a_skill_name_is_refused_before_any_path_is_built() {
        let dir = TempDir::new();
        // A file one level above the skills directory, which `..` would reach.
        dir.write(".yaca/secret.md", "тайна");
        let (ctx, _ws) = ctx_for(&dir);

        for name in [
            "../secret",
            "../../etc/passwd",
            "a/b",
            "/etc/passwd",
            "",
            "Upper",
            "a b",
        ] {
            let err = Skill.run(&ctx, json!({ "name": name })).await.unwrap_err();
            assert!(matches!(err, ToolError::Rejected(_)), "{name:?}: {err:?}");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_skill_that_links_out_of_the_project_is_not_read() {
        let dir = TempDir::new();
        let outside = TempDir::new();
        outside.write("SKILL.md", "---\ndescription: снаружи\n---\nтекст снаружи");
        std::fs::create_dir_all(dir.path().join(".yaca/skills")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(".yaca/skills/linked")).unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        assert!(Skill.run(&ctx, json!({ "name": "linked" })).await.is_err());
        assert!(scan(&ws(&dir)).is_empty());
    }

    #[tokio::test]
    async fn a_body_past_the_cap_is_cut_and_marked() {
        let dir = TempDir::new();
        skill(
            &dir,
            "big",
            &format!("---\ndescription: d\n---\n{}", "a".repeat(40 * 1024)),
        );
        let (ctx, _ws) = ctx_for(&dir);

        let out = Skill.run(&ctx, json!({ "name": "big" })).await.unwrap();
        assert!(out.meta.truncated);
        assert!(out.text.contains("cut here"), "{}", out.text.len());
        assert!(out.text.len() <= MAX_BODY_BYTES + 200);
    }

    #[test]
    fn the_tool_is_offered_in_plan_and_agent_alike() {
        let registry = ToolRegistry::standard();
        assert!(registry.get("Skill", Intent::Plan).is_some());
        assert!(registry.get("Skill", Intent::Agent).is_some());
    }

    #[test]
    fn the_skill_name_rule_is_lowercase_digits_and_hyphens() {
        assert!(is_skill_name("release-2"));
        assert!(is_skill_name("a"));
        assert!(!is_skill_name(""));
        assert!(!is_skill_name("Release"));
        assert!(!is_skill_name("a_b"));
        assert!(!is_skill_name("a.b"));
        assert!(!is_skill_name(&"a".repeat(MAX_NAME_CHARS + 1)));
    }
}
