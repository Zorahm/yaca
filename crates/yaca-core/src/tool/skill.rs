//! The `Skill` tool: hands the model a skill's instructions, or one of its files.
//!
//! The skills themselves — where they are found, how their headers are read,
//! what the model is handed — are `crate::skills`. This is only the model's
//! way in.
//!
//! The catalog is scanned again on every call rather than taken from the
//! session's start. The list in the prompt waits for the next session, because
//! the prompt is the cached prefix; the tool need not, so a skill written during
//! the session works as soon as it is named.
//!
//! Read-only, so offered in Plan as well as Agent and asked nothing: a skill is
//! text, and reading it changes nothing. What the skill then tells the model to
//! do goes through the permissions like everything else.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Narration, Tool, ToolCtx, ToolOutput};
use crate::error::ToolError;
use crate::intent::Intent;
use crate::skills::activate::{self, MAX_RESOURCE_BYTES};
use crate::skills::Catalog;
use crate::tool::memory::store::head;
use crate::workspace::roots::plain;

pub struct Skill;

/// The names a failure tells the model about: those it may call.
fn names_for_model(catalog: &Catalog) -> String {
    let names: Vec<&str> = catalog
        .skills()
        .iter()
        .filter(|s| s.model_invocable)
        .map(|s| s.name.as_str())
        .collect();
    if names.is_empty() {
        "There are no skills.".to_string()
    } else {
        format!("Skills that exist: {}.", names.join(", "))
    }
}

#[async_trait]
impl Tool for Skill {
    fn name(&self) -> &'static str {
        "Skill"
    }

    fn description(&self) -> &'static str {
        "Load a skill: instructions kept for one kind of task, listed by name in the skills \
         catalog. When a task matches a listed skill, call this with its name before you start \
         the task, and follow what it returns. The result gives the skill's directory and lists \
         its other files; call again with `file` to read one of them when the instructions \
         point to it. A name no skill has fails and says which names exist."
    }

    fn properties(&self) -> Value {
        json!({
            "name": {
                "type": "string",
                "description": "The skill's name, as the catalog gives it.",
            },
            "args": {
                "type": "string",
                "description": "For a skill that runs in a subagent of its own: what it is \
                                to work on, passed to it with the instructions.",
            },
            "file": {
                "type": "string",
                "description": "A file of the skill to read instead of its instructions, \
                                relative to the skill's directory, e.g. `references/api.md`. \
                                Leave it out to load the instructions.",
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
        &[Intent::Plan, Intent::Agent]
    }

    fn concurrent(&self) -> bool {
        true
    }

    fn target(&self, input: &Value) -> Option<String> {
        let name = input["name"].as_str()?;
        Some(match input["file"].as_str() {
            Some(file) => format!("{name}/{file}"),
            None => name.to_string(),
        })
    }

    async fn run(&self, ctx: &ToolCtx, input: Value) -> Result<ToolOutput, ToolError> {
        let name = input["name"]
            .as_str()
            .ok_or_else(|| ToolError::failed("missing name"))?;

        let catalog = ctx.skills.scan();
        let Some(skill) = catalog.get(name).filter(|s| s.model_invocable) else {
            return Err(ToolError::failed_with(
                format!("skill `{name}` does not exist"),
                format!(
                    "No skill named `{name}`. {} Call Skill with one of those names, or carry \
                     on without a skill if none of them fits the task.",
                    names_for_model(&catalog)
                ),
            ));
        };

        let Some(file) = input["file"].as_str() else {
            let (text, cut) = activate::content(skill).map_err(ToolError::failed)?;
            // `context: fork`: a subagent follows the skill and reports. A
            // subagent's own call cannot start another, and loads it inline.
            if let (true, Some(agents)) = (skill.fork, ctx.agents.as_ref()) {
                let args = input["args"].as_str().unwrap_or_default().trim();
                let task = if args.is_empty() {
                    format!("{text}\n\nFollow the skill above.")
                } else {
                    format!("{text}\n\nFollow the skill above for: {args}")
                };
                let report = agents.run(task, skill.agent.clone(), None, &ctx.cancel).await?;
                return Ok(ToolOutput::new(format!(
                    "The skill `{name}` ran in a subagent of its own. Its report:\n{report}"
                )));
            }
            return Ok(ToolOutput::new(text).truncated(cut));
        };

        let path = activate::resource_path(skill, file).map_err(ToolError::rejected)?;
        let (text, cut) = head(&path, MAX_RESOURCE_BYTES)
            .ok_or_else(|| ToolError::failed(format!("failed to read {}", plain(&path))))?;
        if text.contains('\0') {
            return Err(ToolError::failed_with(
                format!("{file} is a binary file"),
                format!(
                    "`{file}` is a binary file and cannot be shown as text. Its absolute path is {}.",
                    plain(&path)
                ),
            ));
        }
        let text = if cut {
            format!(
                "{text}\n\n[Only the first {MAX_RESOURCE_BYTES} bytes are shown. The file is {}.]",
                plain(&path)
            )
        } else {
            text
        };
        Ok(ToolOutput::new(text).truncated(cut))
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
        "---\nname: release\ndescription: Cut a release\n---\n\nTag, then build.\n"
    }

    #[tokio::test]
    async fn the_tool_returns_the_body_wrapped_with_its_directory() {
        let dir = TempDir::new();
        skill(&dir, "release", release());
        dir.write(".yaca/skills/release/scripts/tag.sh", "git tag");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Skill.run(&ctx, json!({ "name": "release" })).await.unwrap();
        assert!(out.text.starts_with("<skill_content name=\"release\">\nTag, then build."), "{}", out.text);
        assert!(out.text.contains("Skill directory: "), "{}", out.text);
        assert!(out.text.contains("<file>scripts/tag.sh</file>"), "{}", out.text);
        assert!(!out.text.contains("<file>SKILL.md</file>"), "{}", out.text);
        assert!(out.text.ends_with("</skill_content>"));
        assert!(!out.meta.truncated);
    }

    #[tokio::test]
    async fn a_file_of_the_skill_is_read_by_its_relative_path() {
        let dir = TempDir::new();
        skill(&dir, "release", release());
        dir.write(".yaca/skills/release/references/steps.md", "1. tag");
        let (ctx, _ws) = ctx_for(&dir);

        let out = Skill
            .run(&ctx, json!({ "name": "release", "file": "references/steps.md" }))
            .await
            .unwrap();
        assert_eq!(out.text, "1. tag");
    }

    #[tokio::test]
    async fn a_file_outside_the_skill_is_refused() {
        let dir = TempDir::new();
        skill(&dir, "release", release());
        dir.write(".yaca/secret.md", "secret");
        let (ctx, _ws) = ctx_for(&dir);

        for file in ["../../secret.md", "/etc/passwd", "C:\\Windows\\win.ini", ""] {
            let err = Skill
                .run(&ctx, json!({ "name": "release", "file": file }))
                .await
                .unwrap_err();
            assert!(matches!(err, ToolError::Rejected(_)), "{file:?}: {err:?}");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_that_links_out_of_the_skill_is_refused() {
        let dir = TempDir::new();
        let outside = TempDir::new();
        outside.write("secret.md", "secret");
        skill(&dir, "release", release());
        std::os::unix::fs::symlink(
            outside.path().join("secret.md"),
            dir.path().join(".yaca/skills/release/leak.md"),
        )
        .unwrap();
        let (ctx, _ws) = ctx_for(&dir);

        let err = Skill
            .run(&ctx, json!({ "name": "release", "file": "leak.md" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Rejected(_)), "{err:?}");
    }

    #[tokio::test]
    async fn an_unknown_name_fails_and_names_the_skills_that_exist() {
        let dir = TempDir::new();
        skill(&dir, "release", release());
        skill(&dir, "audit", "---\ndescription: audit\n---\ntext");
        let (ctx, _ws) = ctx_for(&dir);

        let err = Skill.run(&ctx, json!({ "name": "deploy" })).await.unwrap_err();
        assert!(matches!(err, ToolError::Failed { .. }), "{err:?}");
        assert!(err.for_model().contains("audit, release"), "{}", err.for_model());
    }

    #[tokio::test]
    async fn a_skill_kept_from_the_model_cannot_be_called_by_it() {
        let dir = TempDir::new();
        skill(
            &dir,
            "deploy",
            "---\ndescription: d\ndisable-model-invocation: true\n---\ntext",
        );
        let (ctx, _ws) = ctx_for(&dir);

        let err = Skill.run(&ctx, json!({ "name": "deploy" })).await.unwrap_err();
        assert!(err.for_model().contains("no skills"), "{}", err.for_model());
    }

    #[tokio::test]
    async fn a_skill_written_during_the_session_is_found() {
        let dir = TempDir::new();
        let (ctx, _ws) = ctx_for(&dir);
        skill(&dir, "release", release());
        assert!(Skill.run(&ctx, json!({ "name": "release" })).await.is_ok());
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
        assert!(out.text.contains("cut here"));
        assert!(out.text.len() <= activate::MAX_BODY_BYTES + 1024);
    }

    #[test]
    fn the_tool_is_offered_in_plan_and_agent_alike() {
        let registry = ToolRegistry::standard();
        assert!(registry.get("Skill", Intent::Plan).is_some());
        assert!(registry.get("Skill", Intent::Agent).is_some());
    }
}
