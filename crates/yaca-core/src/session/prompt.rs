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

use crate::instructions::Instructions;
use crate::intent::Intent;

/// Which shell the `bash` tool runs commands through.
///
/// An environment fact, so it sits with the project root rather than inside the
/// tool schema: the model reads the prompt before it writes a command, and
/// learning this from a failed `ls` costs a whole turn. Fixed at compile time,
/// which keeps the cached prefix stable.
#[cfg(windows)]
const SHELL_FACT: &str = "Commands run through cmd.exe, not a POSIX shell. Write cmd \
     syntax — `dir`, `type`, `&` and `&&` to chain, `%VAR%` for variables — not `ls`, \
     `cat` or `$VAR`. For looking at files prefer Read, Glob and Grep: they \
     behave the same everywhere.\n";
#[cfg(unix)]
const SHELL_FACT: &str = "Commands run through /bin/sh -c.\n";

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

const GROUNDING: &str = "Ground what you say about this project in files you have \
     read and commands you have run in this conversation, not in how such projects \
     usually look. When a detail is missing, find it with your tools rather than \
     guess, and before saying something does not exist, search for it.\n\n";

const SCOPE: &str = "Deliver what was asked, at the scope intended. Make routine \
     judgment calls yourself, and ask only when different readings of the request \
     would lead to materially different work. If the request looks mistaken or a \
     better approach exists, say so in a sentence and carry on as asked. Anything \
     else you notice on the way — a nearby bug, a cleanup, a missing test — belongs \
     in your final message as a suggestion, not in this change.\n\n";

const CODE: &str = "Write code that reads like the code around it: its naming, idiom \
     and comment density, and the libraries it already uses. Keep the solution as \
     simple as the task allows, without abstractions, options or fallbacks it does \
     not need. Solve the problem in general rather than for the cases the tests \
     check; if a test looks wrong, say so instead of working around it.\n\n";

const BATCHING: &str = "When several reads or searches do not depend on each other, \
     request them together in one response: they run at the same time. A call that \
     needs an earlier result waits for it rather than guessing its value.\n\n";

const FOLLOW_THROUGH: &str = "A step you have decided on is something to do, not to \
     announce: make the call instead of ending your turn with what you will do \
     next. Stop for the human only when the work needs them — an action that is \
     destructive or hard to undo, a real change of scope, or information only they \
     have.\n";

/// Fetching a page waits for permission in every mode, so the rule that governs
/// a refusal cannot live in the agent-only part.
const PERMISSION_NETWORK: &str = "Some actions wait for the human's permission before \
     they run, and reaching the network is one of them.";

const PERMISSION_AGENT: &str = " So do editing, deleting and moving a file, and \
     running a command.";

const PERMISSION_OUTCOME: &str = " You are told only the outcome. If a request is \
     refused with a note, the note is the instruction — follow it instead of trying \
     the same thing again.\n";

const JUDGMENT: &str = "\nPermission is not a substitute for judgment: the human \
     approves what you propose, so propose the reversible way. When something is in \
     your way, deal with its cause rather than going around it by skipping hooks or \
     checks, discarding changes you did not make, or forcing a push. Commit, push or \
     create branches only when the human asks.\n";

const EDITING: &str = "<editing>\nYour edits go to the working tree as you make them, \
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
     Long conversations are compacted automatically when the context fills up, so \
     the length of a conversation is never a reason to stop early or to suggest \
     starting over.\n</communication>\n\n";

const CAPTIONS: &str = "<action_captions>\nSome tools carry a required `description` \
     field. That string is what the human sees in the feed in place of the tool \
     name. Write it about the substance: what you found out or accomplished, not \
     which tool you reached for. Past tense, first person, at most 60 characters, \
     no trailing period, and in the language the human is writing to you in.\n\
     Good: \"Found the existing clipboard usage\".\n\
     Bad: \"Ran rg clipboard src/\".\n\
     Tools without that field are captioned by the interface from the facts — do \
     not narrate those in prose as well.\n</action_captions>\n";

pub fn build(project_root: &Path, intent: Intent, instructions: Option<&Instructions>) -> String {
    let agent = intent == Intent::Agent;
    let mut s = String::from(INTRO);

    s.push_str("<environment>\n");
    s.push_str(&format!(
        "Project root: {}. Write paths relative to it.\n",
        shown(project_root)
    ));
    // No bash tool in plan, so the fact would be noise in the cached prefix.
    if agent {
        s.push_str(SHELL_FACT);
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

    s
}

fn project_section(instructions: &Instructions) -> String {
    let path = &instructions.path;
    let mut s = format!(
        "The human keeps standing instructions for this project in {path}; they \
         follow below. They describe how this project wants work done: follow them, \
         and where they differ from the defaults above, they take precedence. The \
         file was read when this session started, so if it changes during the \
         session, read it again.\n"
    );
    if instructions.truncated {
        s.push_str("The file is long and only its beginning is included here; read it for the rest.\n");
    }
    s.push_str(&format!(
        "<project_instructions path=\"{path}\">\n{}\n</project_instructions>\n",
        instructions.text.trim_end()
    ));
    s
}

/// The root as a person would write it.
///
/// `canonicalize` gives Windows paths a verbatim prefix, `\\?\D:\Kod`, which
/// cmd.exe refuses as a working directory and which the model would copy into
/// its commands.
fn shown(root: &Path) -> String {
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

    fn plain(intent: Intent) -> String {
        build(Path::new("/p"), intent, None)
    }

    fn with_file(text: &str, truncated: bool) -> String {
        let instructions = Instructions {
            path: "CLAUDE.md".into(),
            text: text.into(),
            truncated,
        };
        build(Path::new("/p"), Intent::Agent, Some(&instructions))
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
        // Правки уезжают на диск сразу, и «нажмите Применить» стало бы
        // указанием на кнопку, которой нет (§16.37).
        let p = plain(Intent::Agent);
        assert!(!p.contains("change set"));
        assert!(!p.contains("on disk yet"));
    }

    #[test]
    fn the_shell_is_named_where_commands_can_be_run() {
        // Without this the model writes `ls -la` on Windows and loses a turn.
        let shell = crate::process::shell::NAME;
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
                    path: "AGENTS.md".into(),
                    text: "rules".into(),
                    truncated: false,
                }),
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

    #[test]
    fn the_root_is_shown_without_the_verbatim_prefix() {
        assert_eq!(shown(Path::new(r"\\?\D:\Kod")), r"D:\Kod");
        assert_eq!(shown(Path::new(r"\\?\UNC\srv\share")), r"\\srv\share");
        assert_eq!(shown(Path::new("/home/p")), "/home/p");
    }
}
