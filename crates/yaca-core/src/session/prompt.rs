//! The system prompt.
//!
//! Written in English regardless of the conversation language: models follow
//! English instructions more reliably, and what the human sees is governed by
//! the instructions inside, not by the language they are in.
//!
//! Yaca talks to any model a provider serves (§10), so nothing here leans on
//! one vendor's quirks: no thinking-block tricks, no per-model tuning. What is
//! left is what holds across models — sections in XML tags, the reason next to
//! each rule, what to do rather than what not to, and a calm tone, since
//! capitalised MUSTs make capable models overdo the thing they guard. Length
//! is spent carefully: a weak local model loses the thread of a long prompt.
//!
//! This text is stable for the whole conversation and sits at the first cache
//! breakpoint, so nothing that changes may go in it — no clock, no counters, no
//! list of open files. A single interpolated timestamp voids the cache on
//! every turn, and a model that binds its reasoning to the exact prefix
//! rejects a history whose prefix moved. The changes it does go through — the
//! mode, an edited `AGENTS.md` on resume, an update — are rare, and
//! `session::binding` drops the reasoning they strand.

use std::path::Path;

use crate::instructions::{Instructions, Nested};
use crate::intent::Intent;
use crate::process::shell::Shell;
use crate::tool::memory::store::MemoryIndex;
use crate::tool::skill::Listing;

/// Which shell the `bash` tool runs commands through.
///
/// An environment fact, so it sits with the project root rather than inside the
/// tool schema: the model reads the prompt before it writes a command, and
/// learning this from a failed `ls` costs a whole turn. The same for the whole
/// session — the shell is settled when it opens (`process::shell`) — which
/// keeps the cached prefix stable.
#[cfg(windows)]
pub(super) fn shell_fact(shell: Shell) -> &'static str {
    use crate::process::shell::PowerShell;

    // The two differ in one thing a model trips on. Told that `&&` is missing
    // it writes `;` and `if ($?)`; left to find out, it loses a turn to a
    // parser error on every chained command.
    match shell.powershell() {
        PowerShell::Windows => {
            "Commands run through Windows PowerShell 5.1 (powershell.exe), not cmd \
             and not a POSIX shell. Write PowerShell: `Get-ChildItem`, `$env:NAME`, \
             single quotes around literal arguments. This version has no `&&` or \
             `||`: chain with `;`, or `a; if ($?) { b }` when the second depends on \
             the first. Programs such as git, cargo and npm are called as usual. \
             For looking at files prefer Read, Glob and Grep: they behave the same \
             everywhere.\n"
        }
        PowerShell::Core => {
            "Commands run through PowerShell 7 (pwsh.exe), not cmd and not a POSIX \
             shell. Write PowerShell: `Get-ChildItem`, `$env:NAME`, single quotes \
             around literal arguments; `&&` and `||` work. Programs such as git, \
             cargo and npm are called as usual. For looking at files prefer Read, \
             Glob and Grep: they behave the same everywhere.\n"
        }
    }
}

#[cfg(unix)]
pub(super) fn shell_fact(_shell: Shell) -> &'static str {
    "Commands run through /bin/sh -c.\n"
}

const INTRO: &str = "You are Yaca, a coding agent working inside an editor, in full \
     view of the person you are working with. Every tool call you make shows up in \
     their feed as it happens, together with its result, so they see what you do \
     without being told.\n\n";

const MODE_PLAN: &str = "<mode>\nMode: plan. You are here to investigate and plan: \
     tools that change files or run commands are not available in this mode. Work \
     out what needs doing and write the plan, keeping the task list current with \
     TodoWrite. When the plan is ready, show it with ExitPlanMode and wait — \
     leaving plan mode is the human's decision, not yours.\n</mode>\n\n";

const MODE_AGENT: &str = "<mode>\nMode: agent. Do the work to the end and verify \
     the result, rather than reporting what you intend to do. When the human asks \
     a question, describes a problem or thinks out loud, the deliverable is your \
     answer: investigate, report what you found and stop, and change files only \
     once they ask for a change. When they do ask for one, make it rather than \
     suggesting it.\n</mode>\n\n";

pub(super) const GROUNDING: &str = "Ground what you say about this project in files you have \
     read and commands you have run in this conversation, not in how such projects \
     usually look. When a detail is missing, find it with your tools rather than \
     guess, and before saying something does not exist, search for it.\n\n";

const SCOPE: &str = "Deliver what was asked, at the scope intended. Make routine \
     judgment calls yourself, and ask only when different readings of the request \
     would lead to materially different work. If the request looks mistaken or a \
     better approach exists, say so in a sentence and carry on as asked. Anything \
     else you notice on the way — a nearby bug, a cleanup, a missing test — belongs \
     in your final message as a suggestion, not in this change.\n\n";

pub(super) const CODE: &str = "Write code that reads like the code around it: its naming, idiom \
     and comment density, and the libraries it already uses. Keep the solution as \
     simple as the task allows, without abstractions, options or fallbacks it does \
     not need. Solve the problem in general rather than for the cases the tests \
     check; if a test looks wrong, say so instead of working around it.\n\n";

pub(super) const BATCHING: &str = "When several reads or searches do not depend on each other, \
     request them together in one response: they run at the same time. A call that \
     needs an earlier result waits for it rather than guessing its value.\n\n";

const FOLLOW_THROUGH: &str = "A step you have decided on is something to do, not to \
     announce: make the call instead of ending your turn with what you will do \
     next. Stop for the human only when the work needs them — an action that is \
     destructive or hard to undo, a real change of scope, or information only they \
     have.\n";

/// Fetching a page waits for permission in every mode, so the rule that governs
/// a refusal cannot live in the agent-only part.
pub(super) const PERMISSION_NETWORK: &str = "Some actions wait for the human's permission before \
     they run, and reaching the network is one of them.";

pub(super) const PERMISSION_AGENT: &str = " So do editing, deleting and moving a file, and \
     running a command.";

pub(super) const PERMISSION_OUTCOME: &str = " You are told only the outcome. If a request is \
     refused with a note, the note is the instruction — follow it instead of trying \
     the same thing again.\n";

pub(super) const JUDGMENT: &str = "\nPermission is not a substitute for judgment: the human \
     approves what you propose, so propose the reversible way. When something is in \
     your way, deal with its cause rather than going around it by skipping hooks or \
     checks, discarding changes you did not make, or forcing a push. Commit, push or \
     create branches only when the human asks.\n";

pub(super) const EDITING: &str = "<editing>\nYour edits go to the working tree as you make them, \
     so a file you have written reads back the way you wrote it, and there is \
     nothing for the human to save or apply. Read a file before editing it, and \
     change it with targeted edits rather than rewriting it whole. Check things with \
     commands where you can, and delete any scratch file or script you do create \
     before you finish.\n</editing>\n\n";

const COMMUNICATION: &str = "<communication>\nWrite to the human in the language they \
     write to you in. Be direct: no preamble, and no restating of what their feed \
     already shows.\n\n\
     On work that takes several steps, say in a sentence what you are about to do \
     before you start, and keep the task list current with TodoWrite. While \
     working, add a short note only when you learn something that changes the \
     picture or you change direction.\n\n\
     Your final message is read by someone who may not have followed every step. \
     Open with the outcome — what you did or found — then give the details that \
     matter for what they do next, in complete sentences, leaving behind the \
     shorthand you used while working. Report results as they are: a failing test \
     with its output, a skipped step as skipped, and \"done\" or \"works\" only when \
     a result from this conversation shows it.\n\n\
     In your first response, name the conversation with SetTitle, alongside \
     your other first calls.\n\n\
     Long conversations are compacted automatically when the context fills up, so \
     the length of a conversation is never a reason to stop early or to suggest \
     starting over.\n</communication>\n\n";

pub(super) const CAPTIONS: &str = "<action_captions>\nSome tools carry a required `description` \
     field. That string is what the human sees in the feed in place of the tool \
     name. Write it about the substance: what you found out or accomplished, not \
     which tool you reached for. Past tense, first person, at most 60 characters, \
     no trailing period, and in the language the human is writing to you in.\n\
     Good: \"Found the existing clipboard usage\".\n\
     Bad: \"Ran rg clipboard src/\".\n\
     Tools without that field are captioned by the interface from the facts — do \
     not narrate those in prose as well.\n</action_captions>\n";

pub fn build(
    project_root: &Path,
    intent: Intent,
    instructions: Option<&Instructions>,
    shell: Shell,
) -> String {
    build_with_memory(project_root, intent, instructions, shell, None, None)
}

/// [`build`] with the project's memory index and skills list, where it has them.
///
/// Both are passed in rather than read here: the prompt is rebuilt for every
/// request, and a read in here would change it the moment a note is written or
/// a skill added mid-session, moving the cached prefix. The caller reads them
/// once when the session starts.
pub fn build_with_memory(
    project_root: &Path,
    intent: Intent,
    instructions: Option<&Instructions>,
    shell: Shell,
    memory: Option<&MemoryIndex>,
    skills: Option<&Listing>,
) -> String {
    let agent = intent == Intent::Agent;
    let mut s = String::from(INTRO);

    s.push_str("<environment>\n");
    s.push_str(&format!(
        "Project root: {}. Write paths relative to it.\n",
        shown(project_root)
    ));
    // No bash tool in plan, so the fact would be noise in the cached prefix.
    if agent {
        s.push_str(shell_fact(shell));
    }
    s.push_str("</environment>\n\n");

    s.push_str(if agent { MODE_AGENT } else { MODE_PLAN });

    s.push_str("<how_to_work>\n");
    s.push_str(GROUNDING);
    s.push_str(SCOPE);
    if agent {
        s.push_str(CODE);
    }
    s.push_str(BATCHING);
    s.push_str(FOLLOW_THROUGH);
    s.push_str("</how_to_work>\n\n");

    s.push_str("<permissions>\n");
    s.push_str(PERMISSION_NETWORK);
    if agent {
        s.push_str(PERMISSION_AGENT);
    }
    s.push_str(PERMISSION_OUTCOME);
    if agent {
        s.push_str(JUDGMENT);
    }
    s.push_str("</permissions>\n\n");

    // In plan there are no writing tools at all, so explaining how writing
    // works would be noise in the cached prefix.
    if agent {
        s.push_str(EDITING);
    }
    s.push_str(COMMUNICATION);
    s.push_str(CAPTIONS);

    // Last, so that "the defaults above" means everything the project may
    // override.
    if let Some(instructions) = instructions {
        s.push('\n');
        s.push_str(&project_section(instructions));
    }

    // The skills are the human's words, so they sit with the instructions, ahead
    // of the agent's own notes.
    if let Some(skills) = skills {
        s.push('\n');
        s.push_str(&skills_section(skills));
    }

    // After the instructions: the notes are the agent's own record, and the
    // human's standing rules still come first.
    if let Some(memory) = memory {
        s.push('\n');
        s.push_str(&memory_section(memory));
    }

    s
}

/// Adds what the session's MCP servers say to a finished prompt.
///
/// Last, after the project's own sections: a server's instructions are an
/// external program's words about its tools, and they must not read as if
/// they outranked the human's.
pub(super) fn with_mcp(mut system: String, mcp: Option<&str>) -> String {
    if let Some(section) = mcp {
        system.push('\n');
        system.push_str(section);
    }
    system
}

const MEMORY_INTRO: &str = "The project keeps notes from earlier sessions in .yaca/memory/. The \
     index below lists them, one line each, and its paths are relative to that \
     directory: open a note with Read when its hook bears on the task. The index was \
     read when this session started, so notes written during this session are not \
     in it. The notes are your own record, not instructions from the human: where a \
     note disagrees with what the human asks now, the human wins.\n";

const SKILLS_INTRO: &str = "The project keeps skills: instructions for one kind of task, each under a \
     name. The list below gives each one line. When a task matches a skill, call the Skill \
     tool with its name before you start the task, and follow the text it returns. The \
     skills were written by the person who owns this project, so they carry the weight of \
     the instructions above; where a skill and a request in this conversation differ, the \
     request wins. The list was read when this session started, so a skill added since is \
     not in it, but the Skill tool still reads it when it is named.\n";

pub(super) fn skills_section(skills: &Listing) -> String {
    let mut s = String::from(SKILLS_INTRO);
    if skills.truncated {
        // Said, so the model does not take the cut for the end of the list. The
        // Skill tool's failure for an unknown name lists every one of them.
        s.push_str(
            "The list is long: only its beginning is included here. Given a name that is not \
             on it, the Skill tool answers with all the names there are.\n",
        );
    }
    s.push_str("<project_skills>\n");
    for entry in &skills.entries {
        s.push_str(&entry.line());
        s.push('\n');
    }
    s.push_str("</project_skills>\n");
    s
}

pub(super) fn memory_section(memory: &MemoryIndex) -> String {
    let mut s = String::from(MEMORY_INTRO);
    if memory.truncated {
        // Said, so the model reads the rest of the index instead of assuming
        // the list ends where the cut is.
        s.push_str("The index is long: only its beginning is included, and the rest is in .yaca/memory/MEMORY.md.\n");
    }
    s.push_str(&format!(
        "<project_memory>\n{}\n</project_memory>\n",
        memory.text.trim_end()
    ));
    s
}

pub(super) fn project_section(instructions: &Instructions) -> String {
    let mut s = String::new();

    if let Some(root) = &instructions.root {
        let path = &root.path;
        s.push_str(&format!(
            "The human keeps standing instructions for this project in {path}; they \
             follow below. They describe how this project wants work done: follow them, \
             and where they differ from the defaults above, they take precedence. The \
             file was read when this session started, so if it changes during the \
             session, read it again.\n"
        ));
        if root.truncated {
            s.push_str(
                "The file is long and only its beginning is included here; read it for the rest.\n",
            );
        }
        s.push_str(&format!(
            "<project_instructions path=\"{path}\">\n{}\n</project_instructions>\n",
            root.text.trim_end()
        ));
    }

    if !instructions.nested.is_empty() {
        s.push_str(NESTED_INTRO);
        for nested in &instructions.nested {
            s.push_str(&nested_section(nested));
        }
    }
    s
}

const NESTED_INTRO: &str = "Some directories keep instructions of their own. Each applies to files \
     under its directory and nowhere else, and there it is more specific than \
     the project's instructions above: where they disagree, the nearer file \
     wins. Read when this session started; if one changes, read it again.\n";

fn nested_section(nested: &Nested) -> String {
    let Nested { dir, file } = nested;
    let path = &file.path;
    if file.text.is_empty() && file.truncated {
        // Past the budget: said, so that the model opens it before working
        // there instead of assuming the directory has no rules.
        return format!(
            "<directory_instructions dir=\"{dir}\" path=\"{path}\" included=\"false\">\n\
             Not included here to keep the prompt small. Read {path} before working under {dir}/.\n\
             </directory_instructions>\n"
        );
    }
    let cut = if file.truncated {
        format!("\n[only the beginning of the file is included; read {path} for the rest]")
    } else {
        String::new()
    };
    format!(
        "<directory_instructions dir=\"{dir}\" path=\"{path}\">\n{}{cut}\n</directory_instructions>\n",
        file.text.trim_end()
    )
}

/// The root as a person would write it.
///
/// `canonicalize` gives Windows paths a verbatim prefix, `\\?\D:\Kod`, which
/// a shell refuses as a working directory and which the model would copy into
/// its commands.
pub(super) fn shown(root: &Path) -> String {
    let s = root.display().to_string();
    if let Some(share) = s.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{share}");
    }
    match s.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_string(),
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instructions::RuleFile;

    fn plain(intent: Intent) -> String {
        build(Path::new("/p"), intent, None, Shell::default())
    }

    fn with_file(text: &str, truncated: bool) -> String {
        let instructions = Instructions {
            root: Some(RuleFile {
                path: "CLAUDE.md".into(),
                text: text.into(),
                truncated,
            }),
            nested: Vec::new(),
        };
        build(Path::new("/p"), Intent::Agent, Some(&instructions), Shell::default())
    }

    #[test]
    fn prompt_is_stable_between_calls() {
        // A clock or a counter leaking in here kills the prompt cache silently.
        assert_eq!(plain(Intent::Agent), plain(Intent::Agent));
        assert_eq!(with_file("rules", false), with_file("rules", false));
    }

    #[test]
    fn each_mode_states_itself_plainly() {
        assert!(plain(Intent::Plan).contains("Mode: plan"));
        assert!(plain(Intent::Plan).contains("ExitPlanMode"));
        assert!(plain(Intent::Agent).contains("Mode: agent"));
    }

    #[test]
    fn writing_is_only_explained_where_it_can_happen() {
        // In plan there are no writing tools at all, so the explanation
        // would be noise in the cached prefix.
        assert!(!plain(Intent::Plan).contains("working tree"));
        assert!(plain(Intent::Agent).contains("working tree"));
    }

    #[test]
    fn the_model_is_not_told_to_wait_for_a_button() {
        // Edits go to disk at once, and «нажмите Применить» would become
        // a pointer to a button that does not exist (§16.37).
        let p = plain(Intent::Agent);
        assert!(!p.contains("change set"));
        assert!(!p.contains("on disk yet"));
    }

    #[test]
    fn the_shell_is_named_where_commands_can_be_run() {
        // Without this the model writes `ls -la` on Windows and loses a turn.
        let shell = Shell::default().name();
        assert!(plain(Intent::Agent).contains(shell));
        // No bash tool in plan, so the fact would be noise in the cached prefix.
        assert!(!plain(Intent::Plan).contains(shell));
    }

    #[test]
    fn the_permission_rule_is_stated_in_every_mode() {
        // web_fetch is offered in both modes, so a refusal can arrive in both,
        // and the model has to know the note is an instruction.
        for intent in [Intent::Plan, Intent::Agent] {
            let p = plain(intent);
            assert!(p.contains("reaching the network"), "{intent:?}");
            assert!(p.contains("the note is the instruction"), "{intent:?}");
        }
        // Deletions and commands exist only in agent, so their line does too.
        assert!(!plain(Intent::Plan).contains("deleting"));
        assert!(plain(Intent::Agent).contains("deleting"));
    }

    #[test]
    fn the_caption_rule_is_present() {
        let p = plain(Intent::Agent);
        assert!(p.contains("description"));
        assert!(p.contains("60"));
        assert!(p.contains("language the human is writing"));
    }

    #[test]
    fn tags_are_balanced_in_every_mode() {
        // An unclosed section swallows everything after it, the project's
        // instructions included.
        for intent in [Intent::Plan, Intent::Agent] {
            let p = build(
                Path::new("/p"),
                intent,
                Some(&Instructions {
                    root: Some(RuleFile {
                        path: "AGENTS.md".into(),
                        text: "rules".into(),
                        truncated: false,
                    }),
                    nested: Vec::new(),
                }),
                Shell::default(),
            );
            for tag in [
                "environment",
                "mode",
                "how_to_work",
                "permissions",
                "communication",
                "action_captions",
                "project_instructions",
            ] {
                let open = p.matches(&format!("<{tag}")).count();
                let close = p.matches(&format!("</{tag}>")).count();
                assert_eq!((open, close), (1, 1), "{intent:?} <{tag}>");
            }
        }
    }

    #[test]
    fn project_instructions_come_last_and_name_their_file() {
        let p = with_file("Use tabs.\n", false);
        let section = p.find("<project_instructions").unwrap();
        assert!(p[section..].contains("Use tabs."));
        assert!(p[..section].contains("CLAUDE.md"));
        assert!(p.trim_end().ends_with("</project_instructions>"));
        assert!(p[..section].contains("take precedence"));
    }

    #[test]
    fn a_cut_file_is_said_to_be_cut() {
        assert!(!with_file("rules", false).contains("only its beginning"));
        assert!(with_file("rules", true).contains("only its beginning"));
    }

    fn nested(dir: &str, text: &str, truncated: bool) -> Nested {
        Nested {
            dir: dir.into(),
            file: RuleFile {
                path: format!("{dir}/AGENTS.md"),
                text: text.into(),
                truncated,
            },
        }
    }

    fn prompt_with(root: Option<&str>, nested: Vec<Nested>) -> String {
        let instructions = Instructions {
            root: root.map(|text| RuleFile {
                path: "AGENTS.md".into(),
                text: text.into(),
                truncated: false,
            }),
            nested,
        };
        build(Path::new("/p"), Intent::Agent, Some(&instructions), Shell::default())
    }

    #[test]
    fn nested_instructions_follow_the_root_and_name_their_directory() {
        let p = prompt_with(
            Some("Root rules."),
            vec![nested("crates/core", "Core rules.\n", false)],
        );
        let root = p.find("Root rules.").unwrap();
        let section = p.find("<directory_instructions dir=\"crates/core\"").unwrap();
        assert!(root < section, "the nearer file comes after the general one");
        assert!(p[section..].contains("Core rules."));
        assert!(p[section..].contains("path=\"crates/core/AGENTS.md\""));
        assert!(p.contains("the nearer file"));
        assert!(p.trim_end().ends_with("</directory_instructions>"));
        assert_eq!(p.matches("<directory_instructions").count(), 1);
        assert_eq!(p.matches("</directory_instructions>").count(), 1);
    }

    #[test]
    fn nested_instructions_alone_make_a_section() {
        let p = prompt_with(None, vec![nested("web", "Web rules.", false)]);
        assert!(!p.contains("<project_instructions"));
        assert!(p.contains("Web rules."));
    }

    #[test]
    fn a_nested_file_left_out_or_cut_says_where_to_read_it() {
        let left_out = nested("a", "", true);
        let p = prompt_with(None, vec![left_out]);
        assert!(p.contains("included=\"false\""));
        assert!(p.contains("Read a/AGENTS.md before working under a/"));

        let p = prompt_with(None, vec![nested("b", "Partial.", true)]);
        assert!(p.contains("Partial."));
        assert!(p.contains("only the beginning of the file is included; read b/AGENTS.md"));
        assert!(!p.contains("included=\"false\""));
    }

    #[test]
    fn the_prompt_with_nested_files_is_stable() {
        let make = || prompt_with(Some("r"), vec![nested("a", "x", false), nested("b", "y", false)]);
        assert_eq!(make(), make());
    }

    #[test]
    fn the_root_is_shown_without_the_verbatim_prefix() {
        assert_eq!(shown(Path::new(r"\\?\D:\Kod")), r"D:\Kod");
        assert_eq!(shown(Path::new(r"\\?\UNC\srv\share")), r"\\srv\share");
        assert_eq!(shown(Path::new("/home/p")), "/home/p");
    }

    fn index(text: &str, truncated: bool) -> MemoryIndex {
        MemoryIndex {
            text: text.into(),
            truncated,
        }
    }

    fn with_memory(intent: Intent, memory: &MemoryIndex) -> String {
        build_with_memory(Path::new("/p"), intent, None, Shell::default(), Some(memory), None)
    }

    #[test]
    fn memory_section_follows_the_project_instructions() {
        let instructions = Instructions {
            root: Some(RuleFile {
                path: "AGENTS.md".into(),
                text: "Use tabs.".into(),
                truncated: false,
            }),
            nested: Vec::new(),
        };
        let memory = index("- [a](a.md) — one\n", false);
        let p = build_with_memory(
            Path::new("/p"),
            Intent::Agent,
            Some(&instructions),
            Shell::default(),
            Some(&memory),
            None,
        );
        let rules_end = p.find("</project_instructions>").unwrap();
        let section = p.find("<project_memory>").unwrap();
        assert!(rules_end < section, "the human's rules come first");
        assert!(p[section..].contains("- [a](a.md) — one"));
        assert!(p.trim_end().ends_with("</project_memory>"));
    }

    #[test]
    fn the_prompt_with_memory_is_stable_between_builds() {
        // The index is an input, not a read: two builds with the same index
        // must be the same bytes, or the cache is lost on every request.
        let memory = index("- [a](a.md) — one\n", false);
        assert_eq!(with_memory(Intent::Agent, &memory), with_memory(Intent::Agent, &memory));
    }

    #[test]
    fn without_memory_the_prompt_is_what_it_was_before() {
        assert!(!plain(Intent::Agent).contains("project_memory"));
        assert_eq!(
            build_with_memory(Path::new("/p"), Intent::Agent, None, Shell::default(), None, None),
            plain(Intent::Agent)
        );
    }

    fn skills_of(names: &[(&str, &str)], truncated: bool) -> Listing {
        Listing {
            entries: names
                .iter()
                .map(|(name, description)| crate::tool::skill::Entry {
                    name: (*name).into(),
                    description: (*description).into(),
                })
                .collect(),
            truncated,
        }
    }

    #[test]
    fn skills_are_listed_by_name_and_described_in_a_tagged_section() {
        let skills = skills_of(&[("audit", "Аудит зависимостей"), ("release", "")], false);
        for intent in [Intent::Plan, Intent::Agent] {
            let p = build_with_memory(
                Path::new("/p"),
                intent,
                None,
                Shell::default(),
                None,
                Some(&skills),
            );
            assert_eq!(p.matches("<project_skills>").count(), 1, "{intent:?}");
            assert_eq!(p.matches("</project_skills>").count(), 1, "{intent:?}");
            assert!(p.contains("- audit — Аудит зависимостей\n"), "{intent:?}");
            assert!(p.contains("- release\n"), "{intent:?}");
            assert!(p.contains("call the Skill"), "{intent:?}");
        }
    }

    #[test]
    fn a_cut_skills_list_says_so() {
        let skills = skills_of(&[("audit", "a")], true);
        let p = build_with_memory(Path::new("/p"), Intent::Agent, None, Shell::default(), None, Some(&skills));
        assert!(p.contains("only its beginning is included"), "{p}");
    }

    #[test]
    fn a_project_without_skills_gets_the_prompt_it_always_had() {
        assert!(!plain(Intent::Agent).contains("project_skills"));
        let p = build_with_memory(Path::new("/p"), Intent::Agent, None, Shell::default(), None, None);
        assert_eq!(p, plain(Intent::Agent));
    }

    #[test]
    fn the_skills_section_is_stable_between_builds() {
        let skills = skills_of(&[("audit", "a"), ("release", "b")], false);
        let make = || {
            build_with_memory(
                Path::new("/p"),
                Intent::Agent,
                None,
                Shell::default(),
                None,
                Some(&skills),
            )
        };
        assert_eq!(make(), make());
    }

    #[test]
    fn a_cut_index_says_where_the_rest_is() {
        let p = with_memory(Intent::Agent, &index("- [a](a.md)", true));
        assert!(p.contains("only its beginning is included"));
        assert!(p.contains(".yaca/memory/MEMORY.md"));
        assert!(!with_memory(Intent::Agent, &index("- [a](a.md)", false))
            .contains("only its beginning"));
    }

    #[test]
    fn memory_is_offered_in_plan_mode_too_and_its_tag_balances() {
        // Read works in plan mode, so the index is useful there as well; the
        // section names Read, not the Memory tool, for that reason.
        for intent in [Intent::Plan, Intent::Agent] {
            let p = with_memory(intent, &index("- [a](a.md)", false));
            assert_eq!(p.matches("<project_memory>").count(), 1, "{intent:?}");
            assert_eq!(p.matches("</project_memory>").count(), 1, "{intent:?}");
        }
    }
}
