<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="public/Yaca/yaca-lockup-dark.svg">
    <img src="public/Yaca/yaca-lockup.svg" alt="Yaca" height="64">
  </picture>
</h1>

<p align="center">
  A desktop coding agent. You describe a task, Yaca inspects the project, edits files, and runs commands, asking for your approval before every irreversible action.
</p>

<p align="center">
  <a href="#license"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Version-1.0.8-blue.svg" alt="Version: 1.0.8">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.85%2B-dea584.svg?logo=rust&logoColor=white" alt="Rust: 1.85+"></a>
  <a href="https://tauri.app/"><img src="https://img.shields.io/badge/Tauri-v2-24C8DB.svg?logo=tauri&logoColor=white" alt="Tauri: v2"></a>
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg" alt="Platform: Windows | macOS | Linux">
</p>

---

The current release is `1.0.8`. Installers (Windows NSIS, macOS dmg, Linux deb and AppImage) come from the release workflow and the GitHub Releases page. Only Windows has been run so far. The macOS and Linux builds are untested, and the browser column is Windows only.

## Features

- **Conversations about code.** Streaming answers, thinking, task lists, token and cost tracking, one history per project.
- **Work in the project.** Read, search, edit and delete files — including PDFs, Word, Excel, PowerPoint and Jupyter notebooks — run shell commands and background processes, and wait for their output instead of polling.
- **Permissions.** Plan, Manual, Auto and Edits modes, from read-only to editing without confirmation. Git commands follow the same rules as other shell commands. In Auto, the Decision model or the session model reviews commands that require approval.
- **Checkpoints.** Snapshots before edits and commands, with rollback; the list survives reloads and restarts.
- **Two profiles.** *Code* for repositories, *Documents* for Word, Excel, PowerPoint and PDF work. Both profiles have access to the bundled document skills.
- **Sessions.** Search, Markdown export and resumption of past conversations.
- **Providers.** OpenAI, Anthropic, Google AI Studio, OpenRouter, DeepSeek, z.ai, a small **Decision** model, and any OpenAI- or Anthropic-compatible server, local ones included. Keys live in the system keyring.
- **Model router.** A small model answers typed questions about each message, and rules pick the model that answers it.
- **Isolation.** A chat can run in its own git worktree without touching your working tree.
- **Extensible.** Skills, Claude Code and Agent Plugins plugins, named subagents, hooks, MCP servers, language servers, project memory and slash commands.
- **Browser column** (Windows only). A real page per chat that the model can open, read, click, type in and screenshot, with the human watching and clicking in the same page.
- **Nine languages** (ru, en, de, es, fr, ja, ko, pt, zh), light and dark themes.
- **Signed updates** from GitHub releases, installed only when you press the button.

## Getting Started

**Windows:** download `Yaca_<version>_x64-setup.exe` from the [Releases](../../releases) page and run it. It installs per user and needs no administrator rights.

**From source** (all platforms) needs [Node.js](https://nodejs.org/) 20+ and [Rust](https://rustup.rs/). From the project root:

```bash
npm install
npm run tauri dev
```

`npm run tauri dev` starts Vite and opens the desktop window; `npm run dev` alone serves the frontend without it. The first build takes a while while cargo compiles the core.

Provider keys can be set in Settings → Providers, or from the command line (the key is read from standard input):

```bash
yaca key set --provider anthropic
```

Each model's context window, in tokens, is set in Settings → Providers. Save the provider and reopen the chat for the context meter and automatic compaction to use the new size; unknown sizes default to 1,000,000 tokens.

## Command Line

The same core runs headless without a window:

```bash
cargo run -p yaca-cli -- --project . "what does the build_body function do?"
```

| Flag | Meaning |
|---|---|
| `--mode plan\|manual\|auto\|edits` | permission mode |
| `--permissions allow\|deny` | how to answer permission prompts (defaults to deny) |
| `--add-dir <path>` | one more directory allowed for operations |
| `--isolate` | run in an isolated git worktree |
| `--resume <id>` | resume a recorded session |
| `--jsonl` | output the raw event stream |

Sessions: `yaca sessions`, `yaca search "query"`, `yaca export <id>`. Servers and extensions: `yaca mcp`, `yaca skills`, `yaca plugins`, `yaca lsp`.

**`yaca serve`** lets a phone reach this machine's projects with no window open: `yaca serve [PROJECT...] [--port <PORT>] [--bind <ADDR>] [--pair]`. Each project is a folder the phone may open chats in; without any, the one given with `-p` is served. The port defaults to the remote setting (47820) and the bind address to `0.0.0.0`. A pairing QR code is shown at start when no phone is paired yet, or always with `--pair`; a code works once and expires after five minutes. The terminal also takes `pair`, `devices`, `revoke <id>`, `help` and `quit`. Chats started there are recorded in the same chats folder as the window's.

## Data Storage

Settings and chats live in one directory: `%LOCALAPPDATA%\Yaca` on Windows, `~/Library/Application Support/Yaca` on macOS, `$XDG_CONFIG_HOME/Yaca` on Linux. The desktop app and `yaca-cli` share it, so a session started in one can be resumed in the other.

```text
%LOCALAPPDATA%\Yaca\
├─ settings.json, providers.json, routing.json, mcp.json, lsp.json, plugins.json
├─ skills\, plugins\, plugin-cache\, plugin-data\, marketplaces\   extensions
├─ browser\                                                         browser profile
└─ chats\<project>-<path-hash>\
   ├─ project.json
   ├─ workspaces\<hash>\    files a document chat made, and its attachments
   └─ <id>.jsonl, <id>.messages.jsonl, <id>.meta.json, <id>.checkpoints.json
```

The project's own `.yaca` directory holds local settings, commands, the checkpoint shadow repository, worktrees and drafts. Chat sessions are not stored there. Deleting a chat removes its journals, drafts and document workspace. For an isolated chat it also removes the worktree in `.yaca/worktrees` and keeps the branch `yaca/<id>` so its commits can still be merged.

Uninstalling with "Delete the application data" removes the whole folder, chats and provider keys from Windows Credential Manager; `.yaca` folders inside projects are left alone. Chats saved by older builds in `<project>/.yaca/sessions` and settings in `%APPDATA%\Yaca` are not migrated, and the uninstaller removes that folder too. Custom chat storage directories are not supported.

## Project Configuration

| What | Where |
|---|---|
| Project settings, hooks, MCP servers, profile | `.yaca/settings.json` |
| Slash commands | `.yaca/commands/` |
| Project skills | `.yaca/skills/`, `.agents/skills/`, `.claude/skills/` |
| User skills | `<config>/skills/`, `~/.agents/skills/`, `~/.claude/skills/` |
| Named subagents | `.yaca/agents/`, `.claude/agents/`, `<config>/agents/`, `~/.claude/agents/` |
| Project MCP servers | `.mcp.json` |
| User MCP servers | `mcp.json` in the app settings directory |
| Providers, router, language servers, plugins | `providers.json`, `routing.json`, `lsp.json`, `plugins.json` in the app settings directory |

Project settings override user-wide ones. Yaca only reads the `.claude` and `.agents` folders, so skills, subagents and plugins already installed for Claude Code work without copying.

## Extending Yaca

**Skills.** A skill is a directory with a `SKILL.md` in the [Agent Skills](https://agentskills.io) format: a `name` and `description` in YAML frontmatter, then instructions and any scripts or references. The model sees a catalog of names and descriptions, loads a matching skill when the task calls for it, and you can also run one with `/name`. Yaca installs its bundled `docx`, `xlsx`, `pptx` and `pdf` skills into `<config>/skills/` on start and updates them when the bundled version changes. The skills build on [Anthropic's Agent Skills](https://github.com/anthropics/skills/tree/main/skills) and Yaca's existing [AgentChat](https://github.com/Zorahm/AgentChat/tree/master/skills) workflows. The `docx` skill includes `scripts/replace.py` for text split across Word runs. A project skill of the same name takes precedence.

**Named subagents.** `Task` can hand work to a subagent stored in Claude Code's format in one of the folders above. Its file sets the instructions, a subset of tools (never more than a subagent may have) and a model; the subagent reports back to the conversation that sent it.

**Hooks.** Commands in `.yaca/settings.json` run around tool calls and at session moments: `preToolUse`, `postToolUse`, `sessionStart`, `userPromptSubmit`, `stop`, `subagentStop`, `preCompact`, `sessionEnd` and `notification`. They can refuse or annotate, never grant permissions. A project hook that fails refuses the call, except `stop` and `subagentStop`, which only add a note.

**Plugins.** Skills, commands, MCP servers and hooks packaged together, in Claude Code's format or [Agent Plugins](https://agent-plugins.org) 1.0. Plugins installed in Claude Code are read in place; others come from a marketplace or a direct address:

```bash
yaca plugins marketplace add owner/repo
yaca plugins install name@marketplace
yaca plugins install <dir|owner/repo|git-url>
```

Installed plugins are enabled by default, and their components are namespaced (`plugin:skill`, `/plugin:command`). Downloaded code is never run during installation; hooks and MCP servers only run inside a chat, under the usual permission rules.

**MCP servers.** Configure them in `mcp.json` (user), `.mcp.json` (project) or `mcpServers` in `.yaca/settings.json`, by `command` for a local stdio server or `url` for Streamable HTTP; `${NAME}` and `${NAME:-default}` substitute environment variables. A project's `.mcp.json` needs explicit approval (`yaca mcp --approve <name>`) because it arrives with the repository. Remote servers that need OAuth are logged in from the `/mcp` panel, and their tokens go to the system keychain.

**Language servers.** Yaca finds `rust-analyzer`, `typescript-language-server`, `pyright-langserver` and `gopls`, and offers to install the ones it is missing. The model uses them for definitions, references, types, diagnostics and project-wide renames, and diagnostics of freshly edited files are appended to the edit result. Custom servers go in `lsp.json`; projects cannot define them, because a cloned repository must not be able to start executables.

## Model Router

The router is a virtual provider (`router`) that picks a real model per message. A small decision model answers the questions (*signals*) you wrote in plain language, and the highest-priority matching rule (*decision*) chooses the model; otherwise the message goes to `default`. In the window it appears under "Router" once `routing.json` is valid, and is set up at the bottom of Settings → Providers; from the command line pick `--provider router`.

`routing.json`, next to `providers.json`:

```json
{
  "name": "auto",
  "decisionModel": { "provider": "decision", "model": "qwen3:1.7b", "timeoutMs": 3000 },
  "default": { "provider": "anthropic", "model": "claude-sonnet-5" },
  "signals": [
    { "name": "needs_reasoning",
      "question": { "type": "noul", "instructions": "Does answering this need multi-step reasoning?" } },
    { "name": "request_kind",
      "question": { "type": "choice", "instructions": "What kind of request is this?",
                    "choices": [ { "key": "code", "description": "Writing, reviewing or debugging code" },
                                 { "key": "chat", "description": "Anything else" } ] } }
  ],
  "decisions": [
    { "name": "hard-code",
      "priority": 200,
      "onUnknown": "noMatch",
      "rules": { "all": [
        { "signal": { "name": "request_kind", "label": "code" } },
        { "signal": { "name": "needs_reasoning", "predicate": { "gte": 0.7 } } }
      ] },
      "model": { "provider": "anthropic", "model": "claude-opus-5" } }
  ]
}
```

Questions are `noul` (yes/no, answered with P(yes)) or `choice`; rules combine signals with `all`, `any` and `not`. An unanswered signal is *unknown*, and `onUnknown` says what that means for the rule. A late or unreadable answer falls back to `default` rather than failing the turn; a file that does not parse, or names a missing provider or model, is reported instead of silently ignored.

## Browser

The globe beside the panel button opens the browser column: one page per chat, shared with you, driven by the model through a built-in `browser` MCP server. Navigate, read, screenshot, click, type, run JavaScript, read the console and network log, or change the viewport. Pages cannot reach the app's commands, and downloads and uploads are blocked. Windows only for now.

## Phone Client

An Android phone can attach to the desktop app's live chats: list projects and chats, open or attach to one, send messages and answer permission requests. The agent still runs on the desktop, and a turn started at the desk can be approved from the phone.

The server is off by default. Enabling it (`remote/settings.json` in the app settings directory, or the `set_remote` command) starts a TLS server whose certificate the phone pins from a pairing QR code; each paired phone then holds its own token, and the desktop keeps only its hash. The default port is 47820. Related commands: `remote_status`, `set_remote(enabled, port)`, `remote_pairing`, `revoke_device(id)`. For a headless machine, use `yaca serve`.

The app is in `android/`: Kotlin, Jetpack Compose and Material 3. `android/core` (wire types, feed reducer, connection client) builds without the Android SDK:

```bash
cd android && ./gradlew -p core test       # core, no SDK
cd android && ./gradlew :app:assembleDebug  # app, needs compileSdk 36
```

The wire format is checked from both sides: the Rust tests write an example of every message and event to `android/core/src/test/resources/fixtures/`, and the Kotlin tests decode and re-encode each line.

## Known Gaps

- **macOS and Linux bundles have never been built or run**, and updates there are untested. The browser column is Windows only: the engine interface is implemented for WebView2.
- **No settings screen for the phone server:** enabling it, choosing a port, showing the QR code and revoking devices work only through the commands above.
- **Phone-opened chats do not appear in the desktop sidebar**, and resuming one from the sidebar opens a second session on its journal.
- **The Android app has never been compiled** (the SDK was not installed when it was written); the core is built and tested, the app only read.
- **The phone has no profile switch**, no whole-file tab in the diff panel, and file downloads and rich previews stay on the desktop.
- **Office previews show extracted text and tables, not the original page or slide layout.** Download keeps the original bytes.
- **Formats from before Office 2007** (`.doc`, `.xls`, `.ppt`) and scanned PDFs need conversion or OCR first.

## Architecture

```
crates/yaca-core/   core: agent loop, tools, permissions, checkpoints, model router
crates/yaca-cli/    headless CLI over the core
crates/yaca-evals/  agent evaluation benchmarks
crates/yaca-remote/ phone client's server (TLS + WebSocket)
src-tauri/          Tauri desktop shell
src/                React and TypeScript UI
```

The core knows nothing about windows or webviews: it consumes commands and emits an event stream, and the UI is a reactive projection of that stream.

## Development

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
npm test
npx tauri build --bundles nsis   # Windows installer in target/release/bundle/nsis/
```

Pushing a `v*` tag (it must equal the version in `src-tauri/tauri.conf.json`) runs `.github/workflows/release.yml`, which builds the installers and creates a draft release. Tags with a suffix are marked as pre-releases.

Coding guidelines are in [CLAUDE.md](CLAUDE.md); open questions are in [TODO.md](TODO.md).

## License

MIT
