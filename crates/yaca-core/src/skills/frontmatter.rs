//! The YAML header of a `SKILL.md`.
//!
//! Real YAML, because skills are written for other clients first: a
//! `description: >` folded over three lines and a nested `metadata` map are
//! common, and a `key: value` line reader gets the first wrong and the second
//! not at all.
//!
//! Lenient where other clients are lenient. The one known break is an unquoted
//! value with a colon in it (`description: Use when: the user asks`), which
//! YAML refuses and the parsers of other clients happen to accept. Such a
//! header is parsed a second time with those values quoted, as the Agent Skills
//! client guide suggests, before it is given up on.

use yaml_rust2::{Yaml, YamlLoader};

/// What a skill's header says that Yaca acts on. Everything else in it —
/// `license`, `compatibility`, `metadata`, `allowed-tools` — is read past.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Claude's `disable-model-invocation`: the skill is left out of the
    /// catalog the model reads, and only a human's `/name` brings it in.
    pub disable_model_invocation: bool,
    /// Claude's `user-invocable`: `false` keeps the skill out of the slash menu.
    pub user_invocable: bool,
    /// Claude's `context: fork`: the skill runs in a subagent of its own.
    pub fork: bool,
    /// Claude's `agent`: which named agent a forked skill runs as.
    pub agent: Option<String>,
}

/// Splits a skill file into its header and its body, or says, in words for the
/// human, why it could not.
pub fn parse(text: &str) -> Result<(Header, &str), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some((block, body)) = crate::slash::split_frontmatter(text) else {
        return Err("no header between --- lines".into());
    };
    let doc = load(block)
        .or_else(|first| load(&quote_colons(block)).map_err(|_| first))
        .map_err(|e| format!("header does not parse as YAML: {e}"))?;
    let Yaml::Hash(_) = doc else {
        return Err("header is not a set of key-value pairs".into());
    };

    let header = Header {
        name: scalar(&doc["name"]),
        description: scalar(&doc["description"]),
        disable_model_invocation: flag(&doc["disable-model-invocation"]).unwrap_or(false),
        user_invocable: flag(&doc["user-invocable"]).unwrap_or(true),
        fork: scalar(&doc["context"]).is_some_and(|c| c.eq_ignore_ascii_case("fork")),
        agent: scalar(&doc["agent"]),
    };
    Ok((header, body))
}

fn load(block: &str) -> Result<Yaml, String> {
    let mut docs = YamlLoader::load_from_str(block).map_err(|e| e.to_string())?;
    // An empty header is an empty map: the fields are then missing, which is a
    // problem the caller reports better than "not YAML".
    Ok(if docs.is_empty() {
        Yaml::Hash(Default::default())
    } else {
        docs.swap_remove(0)
    })
}

/// A field as one line of text. Numbers and booleans count: `name: 2024` is a
/// name, whatever YAML thinks it is.
fn scalar(value: &Yaml) -> Option<String> {
    let text = match value {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(n) => n.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        _ => return None,
    };
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// A yes/no field. A quoted `"true"` is accepted, since headers hand-written
/// for other clients carry both.
fn flag(value: &Yaml) -> Option<bool> {
    match value {
        Yaml::Boolean(b) => Some(*b),
        Yaml::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// The header with every top-level unquoted value that holds `": "` put in
/// double quotes. Only the cheap, common case: a value on the key's own line.
fn quote_colons(block: &str) -> String {
    let mut out = String::with_capacity(block.len() + 16);
    for line in block.split_inclusive('\n') {
        let (content, end) = match line.strip_suffix("\r\n") {
            Some(c) => (c, "\r\n"),
            None => match line.strip_suffix('\n') {
                Some(c) => (c, "\n"),
                None => (line, ""),
            },
        };
        match quotable(content) {
            Some((key, value)) => {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                out.push_str(&format!("{key}: \"{escaped}\"{end}"));
            }
            None => out.push_str(line),
        }
    }
    out
}

/// `key` and `value` of a top-level `key: value` line whose value would break
/// YAML for having a colon in it, and is not already quoted or a block.
fn quotable(line: &str) -> Option<(&str, &str)> {
    if line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    let (key, value) = line.split_once(": ")?;
    let value = value.trim();
    let starts_special = value.starts_with(['"', '\'', '|', '>', '[', '{', '&', '*', '!']);
    (!starts_special && value.contains(": ")).then_some((key.trim_end(), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(text: &str) -> Header {
        parse(text).unwrap().0
    }

    #[test]
    fn the_required_fields_and_the_body_come_apart() {
        let (h, body) = parse("---\nname: pdf\ndescription: Read PDFs.\n---\n\n# PDF\n").unwrap();
        assert_eq!(h.name.as_deref(), Some("pdf"));
        assert_eq!(h.description.as_deref(), Some("Read PDFs."));
        assert!(h.user_invocable);
        assert!(!h.disable_model_invocation);
        assert_eq!(body.trim(), "# PDF");
    }

    #[test]
    fn a_folded_description_over_several_lines_is_one_line_of_text() {
        let h = header("---\nname: pdf\ndescription: >\n  Extract text\n  from PDFs.\n---\n");
        assert_eq!(h.description.as_deref(), Some("Extract text from PDFs."));
    }

    #[test]
    fn nested_metadata_and_unknown_fields_do_not_get_in_the_way() {
        let h = header(
            "---\nname: pdf\ndescription: d\nlicense: MIT\nmetadata:\n  author: x\n  version: \"1.0\"\nallowed-tools: Bash(git:*) Read\n---\n",
        );
        assert_eq!(h.name.as_deref(), Some("pdf"));
    }

    #[test]
    fn an_unquoted_colon_in_a_value_is_forgiven() {
        let h = header("---\nname: pdf\ndescription: Use this skill when: the user asks about PDFs\n---\n");
        assert_eq!(
            h.description.as_deref(),
            Some("Use this skill when: the user asks about PDFs")
        );
    }

    #[test]
    fn the_claude_invocation_flags_are_read() {
        let h = header("---\nname: a\ndescription: d\ndisable-model-invocation: true\nuser-invocable: \"false\"\n---\n");
        assert!(h.disable_model_invocation);
        assert!(!h.user_invocable);
    }

    #[test]
    fn a_header_written_on_windows_with_a_bom_is_read() {
        let h = header("\u{feff}---\r\nname: a\r\ndescription: d\r\n---\r\nbody");
        assert_eq!(h.description.as_deref(), Some("d"));
    }

    #[test]
    fn no_header_and_broken_yaml_are_reasons_not_headers() {
        assert!(parse("# just a body").is_err());
        assert!(parse("---\nname: a\n").is_err(), "an unclosed header");
        assert!(parse("---\nname: [a\n---\n").is_err());
        assert!(parse("---\n- a\n- b\n---\n").is_err(), "a list, not fields");
    }

    #[test]
    fn an_empty_header_has_no_fields() {
        let h = header("---\n---\nbody");
        assert_eq!(h.name, None);
        assert_eq!(h.description, None);
    }
}
