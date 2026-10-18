//! Handing a skill to the model: its body, where it lives, what else is there.
//!
//! The body arrives wrapped in `<skill_content name="…">`. The tag is what lets
//! the rest of the core find a skill in the history again: compaction looks for
//! it to bring the instructions of an activated skill back after the summary
//! (`session::compaction`), since a skill that silently drops out of the window
//! leaves an agent that carries on without the guidance and never says so.
//!
//! The files beside `SKILL.md` are listed, not read: the model asks for the one
//! the instructions point it to, through the `Skill` tool's `file`. The skill's
//! directory is usually outside the project, where the file tools do not reach,
//! and opening the workspace boundary for it would let the edit tools reach it
//! too. Reading through the tool keeps the access to what the skill holds.

use std::path::{Component, Path, PathBuf};

use super::catalog::{Skill, FILE};
use crate::tool::memory::store::head;
use crate::workspace::roots::plain;

/// What the model reads of a body. A skill is a set of instructions, not a
/// document; past this it is a document, and the model should read it in parts.
pub const MAX_BODY_BYTES: usize = 32 * 1024;
/// The most of a skill file read at all.
const MAX_FILE_BYTES: usize = 64 * 1024;
/// The most of one resource file handed over.
pub const MAX_RESOURCE_BYTES: usize = 64 * 1024;
/// The files listed beside the instructions. A skill with more is a project of
/// its own, and the model can ask for a directory it was told exists.
const MAX_RESOURCES: usize = 50;
const MAX_RESOURCE_DEPTH: usize = 4;

const OPEN: &str = "<skill_content name=\"";

/// A skill's text as the model receives it, and whether something was cut.
pub fn content(skill: &Skill) -> Result<(String, bool), String> {
    let (text, file_cut) =
        head(&skill.file(), MAX_FILE_BYTES).ok_or_else(|| format!("cannot read {}", plain(&skill.file())))?;
    let body = match super::frontmatter::parse(&text) {
        Ok((_, body)) => body.trim(),
        Err(_) => text.trim(),
    };
    // A plugin's skill finds the scripts it ships through its root.
    let body = match &skill.plugin_root {
        Some(root) => crate::plugin::claude::substitute(body, &plain(root)),
        None => body.to_string(),
    };
    let (body, body_cut) = capped(&body);
    let body = if body.is_empty() {
        "(The skill's file has no text after its header.)".to_string()
    } else {
        body
    };

    let mut out = format!("{OPEN}{}\">\n{body}\n\n", skill.name);
    out.push_str(&format!(
        "Skill directory: {}\nRelative paths in this skill are relative to that directory. Read a \
         file of the skill with the Skill tool's `file` parameter; run its scripts by absolute path.\n",
        plain(&skill.dir)
    ));
    let (files, more) = resources(&skill.dir);
    if !files.is_empty() {
        out.push_str("\n<skill_resources>\n");
        for file in &files {
            out.push_str(&format!("  <file>{file}</file>\n"));
        }
        if more {
            out.push_str(&format!("  (only the first {MAX_RESOURCES} files are listed)\n"));
        }
        out.push_str("</skill_resources>\n");
    }
    out.push_str("</skill_content>");
    Ok((out, file_cut || body_cut))
}

/// What a human's `/name args` gives the model: the skill, then the arguments
/// where the body asks for them or after it.
pub fn invoked(skill: &Skill, args: &str) -> Result<String, String> {
    let (text, _) = content(skill)?;
    Ok(crate::slash::fill(&text, args))
}

/// The skill a typed `/name args` stands for, as the model is to receive it,
/// or `None` when no skill a human may call has that name. The project's own
/// commands are looked up first (`slash::expand`), so a command shadows a skill.
pub fn expand(roots: &super::SkillRoots, text: &str) -> Option<String> {
    let (name, args) = crate::slash::split(text)?;
    let catalog = roots.scan();
    let skill = catalog.get(name).filter(|s| s.user_invocable)?;
    if skill.fork {
        // Run where it belongs, in a subagent, by the tool that can start
        // one; the model only passes it on.
        let with = if args.trim().is_empty() {
            String::new()
        } else {
            format!(" with args `{}`", args.trim())
        };
        return Some(format!(
            "The user invoked the skill `{name}`{with}. It runs in a subagent of its own: call \
             the Skill tool with name `{name}`{with}, and tell the user what its report says."
        ));
    }
    invoked(skill, args).ok()
}

/// Why a path given as a skill's file is refused, or the path under the skill.
pub fn resource_path(skill: &Skill, raw: &str) -> Result<PathBuf, String> {
    let rel = Path::new(raw);
    let plain_parts = rel.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if raw.is_empty() || !plain_parts {
        return Err(format!("{raw} is not a path inside skill: relative path without .. only"));
    }
    let path = std::fs::canonicalize(skill.dir.join(rel))
        .map_err(|_| format!("skill {} has no file {raw}", skill.name))?;
    // After links are followed: a file that is a link out of the skill is not
    // the skill's file.
    if !path.starts_with(&skill.dir) {
        return Err(format!("{raw} leads outside skill {}", skill.name));
    }
    if !path.is_file() {
        return Err(format!("{raw} in skill {} is not a file", skill.name));
    }
    Ok(path)
}

/// The names of the skills whose content appears in `text`.
pub fn activated_in(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices(OPEN).filter_map(|(at, _)| {
        let rest = &text[at + OPEN.len()..];
        rest.find('"').map(|end| &rest[..end])
    })
}

fn capped(body: &str) -> (String, bool) {
    if body.len() <= MAX_BODY_BYTES {
        return (body.to_string(), false);
    }
    let mut cut = MAX_BODY_BYTES;
    while !body.is_char_boundary(cut) {
        cut -= 1;
    }
    let text = format!(
        "{}\n\n[The skill is longer than {MAX_BODY_BYTES} bytes and is cut here. Read the rest from its SKILL.md with the `file` parameter.]",
        body[..cut].trim_end()
    );
    (text, true)
}

/// The skill's other files, relative and with forward slashes, in name order,
/// and whether there were more than are listed.
fn resources(dir: &Path) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let mut more = false;
    walk(dir, dir, 0, &mut out, &mut more);
    out.sort();
    (out, more)
}

fn walk(base: &Path, dir: &Path, depth: usize, out: &mut Vec<String>, more: &mut bool) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "node_modules" || name == "__pycache__" {
            continue;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth + 1 < MAX_RESOURCE_DEPTH {
                walk(base, &path, depth + 1, out, more);
            }
        } else if depth > 0 || name != FILE {
            if out.len() == MAX_RESOURCES {
                *more = true;
                return;
            }
            let rel = path.strip_prefix(base).unwrap_or(&path);
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activations_are_found_by_their_tag() {
        let text = "x <skill_content name=\"pdf\">a</skill_content> y <skill_content name=\"git-flow\">";
        assert_eq!(activated_in(text).collect::<Vec<_>>(), ["pdf", "git-flow"]);
        assert_eq!(activated_in("no skills here").count(), 0);
    }
}
