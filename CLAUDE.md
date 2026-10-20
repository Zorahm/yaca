# Working on Yaca

Yaca is a desktop coding agent: a headless Rust core (`yaca-core`) wrapped by a
Tauri 2 shell (`src-tauri`) with a React 19 + TypeScript UI (`src`). The UI is a
pure function of the core's event stream. Current version: `1.0.0-beta`.

The project was renamed from **edoC** to **Yaca**. The old name survives only in
the migration of data written by earlier builds: `crates/yaca-core/src/legacy.rs`
and `src/core/legacy.ts`. Everything else says Yaca.

## Layout

```
crates/yaca-core/    headless core: agent loop, tools, permissions, checkpoints,
                     model router, MCP, LSP, plugins, skills, hooks, store
crates/yaca-host/    commands and event bridge between the core and a shell (no window in it)
crates/yaca-server/  the host behind a WebSocket + token (`yaca-server`), for remote windows
crates/yaca-cli/     CLI over the core (anything the core does must run from here)
crates/yaca-evals/   agent evaluation benchmarks
src-tauri/           Tauri shell: desktop window around the host, browser pane, toasts;
                     on Android a bare webview with no host in it
src/core/            frontend client, event reducer (feed.ts), generated bindings
src/ui/              React components and CSS; tokens live in src/ui/tokens.css
src/i18n/            dictionaries: ru (source of keys), en, de, es, fr, ja, ko, pt, zh
DESIGN.md PRODUCT.md visual system and product intent (kept in sync with the code)
TODO.md              open questions that wait for a human decision
```

## Commands

```bash
npm ci                                   # frontend deps
npm test                                 # vitest (src/**/*.test.ts)
npm run build                            # tsc + vite build
cargo test --workspace --no-fail-fast    # also regenerates src/core/bindings/
cargo clippy --workspace --all-targets   # must be warning-free
npm run tauri dev                        # dev window (starts vite via beforeDevCommand + core + Tauri)
npx tauri build --bundles nsis           # Windows installer -> target/release/bundle/nsis/
```

- `src/core/bindings/*.ts` is generated from Rust by `ts-rs` during `cargo test`.
  Never edit it by hand; change the Rust type and rerun the tests.
- The disk (C:) runs low. Before a big build check free space; below ~5 GB do
  not start a workspace build. Use `CARGO_INCREMENTAL=0` and no debuginfo on the
  workspace crates (`--config "profile.dev.package.yaca-core.debug=0"`, same for
  `yaca-cli`, `yaca-evals`, `yaca-desktop`). Run integration tests in batches and
  delete `target/debug/deps/*.pdb` between them (safe, cargo does not rebuild
  for it). `cargo clean -p yaca-core -p yaca-cli -p yaca-evals -p yaca-desktop`
  frees ~17 GB and keeps compiled dependencies.
- Run the test suite more than once: tests run in parallel and temp directories
  need truly unique names (Windows clocks are coarse enough to repeat a nanosecond).

## Architecture rules

- `yaca-core` knows nothing about windows, webviews or Tauri. In: `Command`.
  Out: `AgentEvent`. If something in the core cannot be exercised from
  `yaca-cli`, UI leaked into it.
- `yaca-host` is just as windowless. A command exists once, there, and is
  reached by name (`Host::call`) from the desktop shell and from `yaca-server`
  alike; the frontend calls it through `src/core/transport.ts` and never asks
  which. What only a window has (browser column, toasts) joins through the
  hooks of `yaca_host::Shell`. Do not add a `#[tauri::command]` for something
  the server would also need.
- State lives in the core and is sent out whole (`ChangeSetUpdated`,
  `ProcessesUpdated`); the UI never accumulates a second copy of it.
- The UI never lies about state: a running tool pulses, a finished one is
  ticked, irreversible actions ask first. A control that silently does nothing
  is not declared at all. No stubs; if something cannot be finished, record the
  gap in `README.md` and in a comment on the spot, and say so.
- Newtypes over bare `String` for distinct ids (`CallId`, `ProcessId`, `TurnId`).
- `unwrap`/`expect` only where an invariant is proven in an adjacent comment.
- One responsibility per file. Past ~500 lines, split before adding a third thing.
- No copy-paste: extract the second occurrence.
- Events added beyond the existing list are marked in the module header of
  `event.rs`; say so when you add one.

## Language

- Code, identifiers, comments, doc comments, test names, error codes: English.
  Anything an LLM reads (system prompt, tool descriptions, JSON Schema field
  descriptions, skills) is English too.
- No hardcoded Russian anywhere in code, comments or messages. The only Russian
  that stays is the `ru` dictionary, real locale data (language names, plural
  rules, the Russian keyboard layout) and tests of Cyrillic/unicode handling.
- Text a human sees in the feed (tool results in an expanded row, core errors)
  is English in the code. Where the UI can localize it, it is substituted from
  the dictionaries by key; never hardcode another language next to it.
- Every user-facing string goes through i18n keys, never hardcoded in JSX.
  Keys are typed from the Russian dictionary (`src/i18n/ru`); a new key needs all
  nine locales, and the compiler catches a missing one.
- Comments explain *why* (the decision, a non-obvious constraint), never what
  the code already says.

## Tests

- The main instrument is golden tests of the event stream: `MockProvider`
  replays a recorded model answer and the test asserts the exact `AgentEvent`
  sequence. Model: `crates/yaca-core/tests/permissions.rs`.
- End-to-end tests go through the public API (`spawn`, `Command`, `AgentEvent`).
- Where a tool touches the filesystem or processes, the test touches them too;
  no filesystem mocks.
- The feed reducer and dictionaries are pure functions tested with vitest.
- Known flaky under load: `commands.rs::a_background_command_that_ends_mid_turn_...`
  (passes alone).

## Design

Read `DESIGN.md` before touching UI. Short version: "graphite and teal" in
`src/ui/tokens.css`; use tokens, never raw colours; one accent (teal) for what is
yours or needs you; motion only as state pulses; density is a principle. The
theme values in code are `paper` (light, default), `night` (dark) and `system`;
these are legacy names, do not rename them without a data migration.

## Windows installer

- NSIS config is in `src-tauri/tauri.conf.json`; Windows-only overrides
  (frameless window) in `src-tauri/tauri.windows.conf.json`.
- `src-tauri/windows/hooks.nsh` must stay UTF-8 **with BOM**, otherwise NSIS
  reads the Russian text in the ANSI code page.
- Installer art is produced by `scripts/installer_art.py`.

## Release

`.github/workflows/release.yml` runs on a `v*` tag, checks that the tag equals
`src-tauri/tauri.conf.json` `version`, runs `npm test`, builds and creates a
**draft** pre-release. Versions live in `Cargo.toml` (workspace), `package.json`,
`package-lock.json`, `Cargo.lock` and `tauri.conf.json`; bump them together.
The matrix builds Windows (NSIS), macOS (dmg, Apple silicon and Intel, unsigned)
and Linux (deb, AppImage) into one draft release. Linux needs `libdbus-1-dev`
even for `yaca-core` (the keyring's Secret Service); the workflow installs it.
The update check picks the asset by file suffix and processor name
(`crates/yaca-core/src/update.rs`), so keep Tauri's default asset names.
Only the core was compiled and tested on Linux (Ubuntu 24.04); the macOS and
Linux bundles, and the Tauri shell off Windows, have never been built.

## Git

The owner's rules apply (load the `zowork:solo-git` skill before any git action):
commit only on an explicit command, push only on an explicit command, no PRs, no
branches. `AGENTS.md` is a symlink to this file; keep it so.

## Working style

- Design talk goes in chat, not files. Do not add plans, specs or reports to the
  repository. `TODO.md` is the exception: open questions that wait for a human.
- When code and a spec disagree, say so in a sentence and follow the user.
- Discuss deviations from the intended behaviour instead of deciding silently.
- Verify before claiming done: run the tests, build the thing, look at the
  result. Report failures with their output.
