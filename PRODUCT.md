# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

A solo developer who keeps the window open all working day and delegates
coding tasks to an agent while watching what it does. The interface speaks nine
languages (Russian, English, German, Spanish, French, Japanese, Korean,
Portuguese, Chinese) and follows the system language by default. (Inferred from
`README.md` and the code; not confirmed in an interview.)

## Product Purpose

Yaca is a desktop coding agent: a Tauri shell over a headless Rust core. The
human describes a task; the agent reads and edits files, runs commands and
asks permission, and every action is visible in a feed as it happens. The same
core runs from a command line (`yaca-cli`) with no window at all.

## Positioning

The interface is a pure function of the core's event stream and never lies
about state: a running tool pulses, a finished one is ticked, stale context is
marked, irreversible actions ask first. There is one confirmation, before the
write. Only the human commits.

## Operating Context

The session window: a left sidebar with projects and chats, a top bar, a
centred feed (human message, agent text, collapsed reasoning, action groups
with diffs and command output, task list, permission request with numbered
answers, checkpoint dividers with rollback, errors, history compaction),
notification bars and a composer with mode, model, effort and a status ring
that opens the context window. Slide-over panels hold processes, a file diff,
checkpoints, providers and each chat's browser. A separate settings window
holds preferences, providers, MCP servers, plugins, skills, language servers and
model routing. The window is frameless on Windows and ships as an NSIS
installer.

## Capabilities and Constraints

- Density is a principle: 14px body, 13px feed and menu text, 12–12.5px mono in
  tools, 22px diff line (20px in compact mode).
- No stubs: a control that silently does nothing is not declared at all.
- Every user-facing string goes through i18n keys.
- Two themes exist, light by default and dark, plus "follow the system".
- Motion is limited to state pulses.
- Any provider speaking the Anthropic or OpenAI protocol, local servers
  included; a model router can pick the model per task.
- Extensible by skills, named subagents, project hooks, plugins, MCP servers
  (with OAuth) and language servers. Each chat gets a browser the model drives.
- Reads PDFs and notebooks; keeps provider keys in the system credential store.

## Brand Commitments

The name is Yaca. The visual system is "graphite and teal": a cool grey ground,
white panels with a hairline border, one teal accent, set in Onest with
JetBrains Mono for code. It replaced the earlier paper-and-red "proof" system.
Restraint in colour is a standing preference: teal marks what is yours or needs
you, failures are a muted brick red, and nothing else carries meaning by hue
(the context-window bar is the one named exception). Multi-colour trials were
rejected as too many colours.

## Product Principles

- The agent works in plain sight.
- One confirmation.
- Density for an all-day tool.
- The interface does not lie about state.
