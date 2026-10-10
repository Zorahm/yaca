# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

A solo developer who keeps the window open all working day and delegates
coding tasks to an agent while watching what it does. The interface is
Russian first, English second. (Inferred from `README.md` and `docs/ui.md`;
not confirmed in an interview.)

## Product Purpose

Yaca is a desktop coding agent: a Tauri shell over a headless Rust core. The
human describes a task; the agent reads and edits files, runs commands and
asks permission, and every action is visible in a feed as it happens.

## Positioning

The interface is a pure function of the core's event stream and never lies
about state: a running tool pulses, a finished one is ticked, stale context is
marked, irreversible actions ask first. There is one confirmation, before the
write. Only the human commits.

## Operating Context

The session window: a left sidebar with projects and sessions, a top bar, a
centred feed (human message, agent text, collapsed reasoning, action groups
with diffs and command output, task list, permission request with numbered
answers, checkpoint dividers with rollback, errors, history compaction),
notification bars and a composer with mode, model, effort and a status ring.
Slide-over panels hold processes, a file diff, checkpoints and providers.

## Capabilities and Constraints

- Density is a principle: 13.5px feed text, 11.5px mono in tools, 22px diff
  line.
- No stubs: a control that silently does nothing is not declared at all.
- Every user-facing string goes through i18n keys.
- Two themes exist, light by default and dark.
- Motion is limited to state pulses.
- Any provider speaking the Anthropic or OpenAI protocol, local servers
  included.
- `docs/ui.md` is the normative interface spec.

## Brand Commitments

The name is Yaca. The visual system is "the proof" (Корректура): warm paper,
black type and one red pencil, set in Onest with JetBrains Mono for code. It
replaced the earlier vermilion-accent system on the
`claude/skill-app-redesign-73f589` branch. Restraint in colour is a standing
preference: two multi-colour trials were rejected as too many colours.

## Product Principles

- The agent works in plain sight.
- One confirmation.
- Density for an all-day tool.
- The interface does not lie about state.
