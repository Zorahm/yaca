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
  <img src="https://img.shields.io/badge/Version-1.0.0--beta-blue.svg" alt="Version: 1.0.0-beta">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.85%2B-dea584.svg?logo=rust&logoColor=white" alt="Rust: 1.85+"></a>
  <a href="https://tauri.app/"><img src="https://img.shields.io/badge/Tauri-v2-24C8DB.svg?logo=tauri&logoColor=white" alt="Tauri: v2"></a>
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg" alt="Platform: Windows | macOS | Linux">
</p>

---

Yaca is in beta (`1.0.0-beta`). Installers (Windows NSIS `.exe`, macOS `.dmg`, Linux `.deb` and `.AppImage`) are built by the release workflow and published on the GitHub Releases page. Only Windows has been run so far; the macOS and Linux builds are untested, and the browser column is Windows only.

## Features

- **Conversations with models about code.** Streaming responses, thinking/reasoning, task lists, token and cost tracking.
- **Project manipulation.** Read, search, edit, and delete files, including the text of PDFs and the cells of Jupyter notebooks; see [PDFs and Notebooks](#pdfs-and-notebooks). Run shell commands and background processes, and wait for a line of their output or for them to end with `BashWait` instead of polling.
- **Permissions.** Modes: "Plan", "Manual", "Auto", and "Edits" — ranging from read-only to editing without confirmation. Git commit and push always require approval.
- **Checkpoints.** Snapshots project state before edits and commands, allowing you to roll back changes. The list of points is saved with the session, so it survives a window reload, closing and reopening the chat, and a restart.
- **Reloading the window does not stop the agent.** Ctrl+R (or a hot reload in development) only redraws the window: running turns, background commands and open questions carry on in the app, and the window picks the chats up again where they are. Restarting the application itself still ends them.
- **Sessions.** Project conversation history, search, Markdown export, and resumption from where you left off.
- **Providers.** Ready to use by just entering an API key: OpenAI, Anthropic, Google AI Studio, OpenRouter, DeepSeek, z.ai (coding plan). Plus **Decision**, a ready entry for the small model that answers the router's questions: point it at a local server (Ollama's address is filled in) or a cloud one. Plus any custom servers compatible with OpenAI or Anthropic API formats, including local ones. After saving a key, models are queried automatically.
- **Model router.** A small decision model answers typed questions about each message, and rules pick which model answers it; see [Model Router](#model-router).
- **Decision tool.** The agent can put a quick question to the small Decision model — which skill fits, which of three files to open — and read its short answer; see [Asking the Decision model](#asking-the-decision-model).
- **Isolation.** Chats can run in their own git worktree without touching your working tree.
- **Skills** in the [Agent Skills](https://agentskills.io) format, including skills already installed for Claude Code; see [Skills](#skills).
- **Plugins** from Claude Code and [Agent Plugins](https://agent-plugins.org): skills, commands, MCP servers, and hooks, including plugins already installed in Claude Code; see [Plugins](#plugins).
- **Subagents**, plain or named, with their own instructions, tools and model; see [Named Subagents](#named-subagents).
- **Hooks** around tool calls and at the session's moments; see [Project Hooks](#project-hooks).
- **Project memory** and custom slash commands from `.yaca/commands/`.
- **MCP.** Tools and resources from external Model Context Protocol servers alongside built-in ones; see [MCP Servers](#mcp-servers).
- **LSP.** Definitions, references, types, diagnostics, and project-wide renames from language servers; see [Language Servers (LSP)](#language-servers-lsp).
- **Interface in nine languages** (Russian, English, German, Spanish, French, Japanese, Korean, Portuguese, Chinese), following the system language by default, with a light and a dark theme.
- **Browser.** A real page per chat in the browser column beside the feed: the model opens it, reads it, takes screenshots, clicks and types, and runs JavaScript in it, and the human watches and can click in the same page. Windows only for now; see [Browser](#browser).

**Updates.** About → Updates checks the GitHub releases by hand, and the app does the same by itself shortly after start and every few hours (switch it off in the same place). A newer release is shown with its changelog, the text of the release, and a download link; nothing is installed automatically. A beta is told about the next beta and the release, a release only about releases (alpha < beta < rc < release, then the numbers). The menu item "Help → Check for Updates" opens "About", where this is explained.

## Getting Started

**Windows:** download `Yaca_<version>_x64-setup.exe` from the [Releases](../../releases) page and run it. It installs per user and needs no administrator rights.

**From source** (all platforms) requires [Node.js](https://nodejs.org/) 20+ and [Rust](https://rustup.rs/).

From the project root:

```bash
npm install
npm run tauri dev
```

`npm run tauri dev` starts the Vite dev server (`npm run dev`) and opens the desktop window. `npm run dev` alone serves only the frontend, without the window.

The initial build takes a while as cargo compiles the core from scratch.

Provider keys can be set in the Settings window or via the command line. Keys are stored in the system keyring and read from standard input:

```bash
yaca key set --provider anthropic
```

## Data Storage

Application settings and chats are stored in a single directory. On Windows, this is `%LOCALAPPDATA%\Yaca` — the same folder where the installer places the app and uninstaller:

```text
%LOCALAPPDATA%\Yaca\
├─ settings.json, providers.json, routing.json, mcp.json, lsp.json, mcp-approved.json, plugins.json
├─ marketplaces.json, installed_plugins.json
├─ skills\, plugins\       user skills and plugins placed manually
├─ marketplaces\<name>\    cloned marketplaces
├─ plugin-cache\           installed plugins
├─ plugin-data\<plugin>\   PLUGIN_DATA for plugins
├─ browser\               the browser column's profile: its cookies and site storage
└─ chats\<project>-<path-hash>\
   ├─ project.json         identifies the project
   └─ <id>.jsonl, <id>.messages.jsonl, <id>.meta.json, <id>.checkpoints.json
```

On macOS, this is `~/Library/Application Support/Yaca`; on Linux, `$XDG_CONFIG_HOME/Yaca`. Both the desktop app and `yaca-cli` read and write to the same location, so a session started in one can be resumed in the other.

The project's own `.yaca` directory holds its local settings, commands, shadow repository for checkpoints, worktrees, and drafts. Chat sessions are no longer stored there.

The shadow repository holds what the snapshots contain; `<id>.checkpoints.json` says which snapshots belong to which chat and in what order. Snapshots a chat's list names are kept for as long as the chat exists, and deleting the chat frees them. A point whose snapshot has gone missing is left out of the list rather than offered as a rollback that cannot work.

When uninstalling the application, checking "Delete the application data" will display a warning and delete the entire `%LOCALAPPDATA%\Yaca` folder along with chats, as well as provider keys (`<provider>.Yaca`) from Windows Credential Manager. It does not touch `.yaca` folders inside projects.

Not supported:

- **Migration of old chats.** Sessions saved by older builds in `<project>/.yaca/sessions` and settings from `%APPDATA%\Yaca` are not migrated automatically. The uninstaller removes that folder as well.
- **Custom chat storage directories.**

## Command Line

The same core runs headless without a GUI:

```bash
cargo run -p yaca-cli -- --project . "what does the build_body function do?"
```

Useful flags:

| Flag | Description |
|---|---|
| `--mode plan\|manual\|auto\|edits` | permission mode |
| `--permissions allow\|deny` | how to answer permission prompts (defaults to deny) |
| `--add-dir <path>` | additional directory allowed for operations |
| `--isolate` | run in an isolated git worktree |
| `--resume <id>` | resume a recorded session |
| `--jsonl` | output raw event stream |

Project sessions: `yaca sessions`, `yaca search "query"`, `yaca export <id>`. Project MCP servers: `yaca mcp`, `yaca mcp --approve <name>`, `yaca mcp --login <name>` (OAuth in the browser), `yaca mcp --logout <name>`. Skills and file diagnostics: `yaca skills`. Plugins and their toggles: `yaca plugins`, `yaca plugins --enable <id>`, `yaca plugins --disable <id>`; for installation, see [Installing Plugins](#installing-plugins).

## Skills

A skill is a directory containing a `SKILL.md` file in the [Agent Skills](https://agentskills.io/specification) format: a YAML frontmatter with `name` and `description`, followed by instructions. It can also include `scripts/`, `references/`, and any other files.

```markdown
---
name: release
description: Release a new version. Use when asked for a release or tag.
---

Tag first, then build. Steps are in references/steps.md.
```

Where Yaca looks for skills, in descending order of precedence:

| Scope | Directories |
|---|---|
| Project | `.yaca/skills/`, `.agents/skills/`, `.claude/skills/` in the project root |
| User | `skills/` in the app settings directory, `~/.agents/skills/`, `~/.claude/skills/` |
| Plugins | skills from enabled [plugins](#plugins), named `plugin:skill` |

A project skill overrides a user skill with the same name; within the same scope, `.yaca` overrides `.agents`, which in turn overrides `.claude`. Yaca only reads these directories, so skills from Claude Code and other clients work without copying.

The model sees a catalog: the name and description of each skill. It loads a matching skill using the `Skill` tool, and reads skill files using the same tool (`file`), even if the skill is located outside the project. Skill scripts run via standard `Bash` with the usual permission prompt. Skills can also be invoked manually via `/name arguments`. A command with the same name in `.yaca/commands/` takes precedence over the skill there too. The list in the prompt is read when opening a chat, while the skill content is read upon invocation, so skill edits take effect immediately, while newly added skills appear in the list starting with the next chat.

During context compaction, previously loaded skills are re-read from disk and placed immediately after the summary, preventing the agent from silently losing their instructions.

Parsing is lenient because most skills are written for other clients. Non-standard names, folder name mismatches, or overly long descriptions produce warnings while the skill is still loaded. Skills lacking a `description` or having a frontmatter that fails YAML parsing are skipped. These issues are surfaced by `yaca skills` and the "Project" section in Settings.

Claude Code fields supported: `disable-model-invocation` (skill excluded from the model catalog, only accessible via `/name`) and `user-invocable: false` (hidden from the slash menu).

Not supported in skills:

- **`allowed-tools`.** The field is parsed but grants no permissions: a skill from a cloned repository should not be able to self-approve commands.
- **Skills from parent directories** up to the git root, or custom skill paths in settings.
- **Claude Code fields `model` and `argument-hint`.** `context: fork` and `agent` are supported: such a skill runs in a subagent of its own (as the named agent `agent`, when given), and the model gets the subagent's report rather than the skill's text. A human's `/name` of a forked skill asks the model to run it that way.
- **Deduplication on reload.** Calling a skill a second time sends the full content to the model again.

## Named Subagents

`Task` hands a task to a subagent: a conversation of its own that reports back. A named subagent is one kept in a file, in Claude Code's format:

```markdown
---
name: reviewer
description: Reviews a diff for real bugs. Use after a change, before committing.
tools: Read, Grep, Glob
model: claude-haiku-4-5
---

You review code. Report only bugs that would break something, with path:line.
```

Where Yaca looks, in precedence order: `.yaca/agents/` and `.claude/agents/` in the project, then `agents/` in the app settings directory and `~/.claude/agents/`, then the `agents/` of enabled plugins (named `plugin:agent`). The first file with a name wins. The list goes into the system prompt when the chat opens, and the model sends one with `Task`'s `agent` parameter.

- **Instructions.** The file's body is added to the subagent's system prompt, after Yaca's own.
- **`tools`** narrows the subagent's tools and never widens them: a file cannot hand a subagent what subagents never get (`Task`, `AskUser`, the task list). Claude Code's names are understood (`MultiEdit` is `Edit`, `PowerShell` is `Bash`), and `mcp__server` covers that server's tools. Without `tools`, the subagent has the usual set.
- **`model`**: the id of any model the chat's provider lists, whoever made it (case does not matter), or `inherit` for the chat's own model. There are no family aliases: Claude Code's `sonnet`, `opus` and `haiku` name nothing here. An id the provider does not list runs on the default subagent model, and the `Task` row says so.

`Task` also takes a `model` of its own, for when the human names one ("check this with gpt-5.1"): the same ids, and it outranks the agent's. There the model passes on the human's id, so an id the provider does not list fails the call and names the ids there are, rather than running on another model.

A name nobody has fails the call and lists the agents there are.

## Project Hooks

Commands in `.yaca/settings.json` that run around tool calls and at the session's moments. The human wrote them, so they run without a permission question; they can refuse or annotate, never grant.

```json
{
  "hooks": {
    "preToolUse": [{ "match": "Bash", "command": "./scripts/guard.sh" }],
    "postToolUse": [{ "match": "Edit", "command": "cargo fmt" }],
    "stop": [{ "command": "cargo test -q" }],
    "notification": [{ "command": "notify-send Yaca 'waiting for you'" }]
  }
}
```

| Key | When | What it can do |
|---|---|---|
| `preToolUse` | Before a call, ahead of the permission question | Exit 0 lets it run; anything else refuses it. `{"updatedInput": {...}}` on stdout rewrites the call's arguments, and the permission question is asked about the rewritten call |
| `postToolUse` | After a call | Its output is added to the result the model reads |
| `sessionStart` | When the chat opens | Its output is standing context in the system prompt |
| `userPromptSubmit` | Before each message reaches the model | Exit 0 adds its output to the message; anything else keeps the message from the model |
| `stop` | When the model finishes its turn | A non-zero exit sends the model back to work with the hook's output, up to 3 times per turn |
| `subagentStop` | When a subagent is about to report | The same, for the subagent |
| `preCompact` | Before history is compacted (`trigger`: `manual` or `auto`) | Told only |
| `sessionEnd` | When the chat closes | Told only |
| `notification` | When Yaca starts waiting for the human: a permission question, a question, an MCP server's form | Told only, and not waited for |

Tool hooks take `match` (`*` or one tool name); the others do not. Each hook gets a JSON description of the event on stdin, `YACA_PROJECT_ROOT` in its environment, and `YACA_TOOL` or `YACA_HOOK_EVENT`. `timeoutSecs` defaults to 30.

Project hooks fail closed: a hook that cannot start or does not finish in time refuses the call, and keeps a message from the model. `stop` and `subagentStop` are the exception: sending the model back to work because a check could not start would give it something it cannot fix, so that is a note instead.

## Plugins

A plugin is a directory containing skills, commands, MCP servers, and hooks. Two formats are supported:

- **Claude Code**: `.claude-plugin/plugin.json`, components in `skills/`, `agents/`, `commands/`, `.mcp.json`, `hooks/hooks.json`, plus paths added by the manifest. Parsed leniently, matching Claude Code: only `name` is required; without a manifest, a plugin installed in Claude Code takes the name under which it was installed.
- **[Agent Plugins](https://agent-plugins.org/specification) 1.0**: `plugin.json` at the root, `skills/`, and `mcp.json`. Parsed strictly according to the specification: an invalid manifest rejects the entire plugin, an invalid `mcp.json` disables the plugin's servers, and an invalid entry disables only itself.

If a plugin contains both manifests, it is loaded as a Claude Code plugin.

Plugin discovery locations:

1. `plugins/<directory>/` in the app settings folder — user plugins installed manually.
2. Installed by Yaca from a marketplace or direct URL; see [Installing Plugins](#installing-plugins).
3. Installed in Claude Code for the user (`scope: user` in `~/.claude/plugins/installed_plugins.json`). Yaca only reads them and never writes to Claude Code directories. If the same plugin exists in both Yaca and Claude Code, the Yaca copy is loaded.

Toggles are stored in `plugins.json` in the settings directory and can be configured in the "Plugins" settings tab or via `yaca plugins --enable|--disable`. A plugin without an explicit toggle inherits its state from Claude Code's `enabledPlugins`, or defaults to enabled if local to Yaca. Changes apply to newly opened chats; an existing chat continues using the set it started with.

Each component is namespaced by plugin prefix to prevent collisions:

| Component | Identifier |
|---|---|
| Skill | `plugin:skill` in the model catalog and `/plugin:skill` |
| Command | `/plugin:command` |
| MCP Server | `plugin:plugin:server`, tools named `mcp__plugin_plugin_server__…` |

MCP servers from enabled plugins start without separate approval: enabling the plugin implies consent. The server starts from the plugin root, with `PLUGIN_ROOT`, `PLUGIN_DATA`, `CLAUDE_PLUGIN_ROOT`, and `CLAUDE_PLUGIN_DATA` in its environment. In skill and command texts, `${CLAUDE_PLUGIN_ROOT}` is replaced with the path to the plugin. `PLUGIN_DATA` uses Yaca's own folder in `plugin-data`, not Claude Code's folder.

### Plugin Hooks

Plugin hooks follow Claude Code rules rather than project hook rules from `.yaca/settings.json`:

| Event | When | Capabilities |
|---|---|---|
| `SessionStart` | On chat opening (`source`: `startup` or `resume`) | Add context: injected into system prompt for the entire session, survives compaction |
| `UserPromptSubmit` | Before each user message | Block message from reaching model, or prepend/append context |
| `PreToolUse` | Before invocation, after project hooks | Deny invocation (exit code 2 or `permissionDecision: "deny"`), or require confirmation (`"ask"`) |
| `PostToolUse` | After invocation | Append a note for the model to the tool result |
| `Stop` | When the model finishes its turn | Resume execution with a custom reason (up to 3 times per turn) |
| `SubagentStop` | When a subagent is about to report | The same, for the subagent |
| `PreCompact` | Before history is compacted (`matcher` against `manual` or `auto`) | Informational |
| `SessionEnd` | When the chat closes | Informational |
| `Notification` | When Yaca starts waiting for the human (`matcher` against `permission_prompt`, `question`, `elicitation`) | Informational, not waited for |

A `PreToolUse` hook may also rewrite the call: `hookSpecificOutput.updatedInput` replaces its arguments, and the permission question is then asked about the rewritten call.

Only exit code 2 or an explicit JSON denial blocks execution. A hook that fails, fails to launch, or times out becomes an informational note, allowing work to proceed (matching typical plugin expectations). In contrast, project hooks deny by default on failure. No hook can grant permissions: `permissionDecision: "allow"` is ignored.

Command strings run via bash (on Windows, via Git Bash, matching Claude Code; configurable via `CLAUDE_CODE_GIT_BASH_PATH`). The `command` + `args` form runs the binary directly. Supported options include `matcher` (regex against Claude Code tool names: `PowerShell` maps to `Bash`, `AskUserQuestion` to `AskUser`), `if`, `timeout`, `async`, and `shell: "powershell"`. Any hook output outside the command invocation line is displayed in the feed as a separate entry.

Not supported in plugins:

- **Language servers and output styles** from plugins. Plugins containing them load, but these features are listed as unsupported.
- **Hook `Setup`**, and hook types `prompt` and `agent`.
- **`permissionDecision: "ask"` for tools without permissions** (`Read`, `Grep`): there is no confirmation prompt to trigger, so the hook response is ignored.
- **Transcripts in Claude Code format.** `transcript_path` points to Yaca's log, which hooks expecting Claude Code format (e.g. claude-mem) will not understand.
- **Project plugins** (`enabledPlugins` in repository `.claude/settings.json`, installations with `scope: project`).
- **`sse` transport** for plugin MCP servers.
- **`$1`, `$2` in plugin commands.** Only `$ARGUMENTS` is supported.

## Installing Plugins

Plugins can be installed from Claude Code format marketplaces (`.claude-plugin/marketplace.json`) — via the "Plugins" settings tab or from the command line:

```bash
yaca plugins marketplace list
yaca plugins marketplace add owner/repo
yaca plugins browse claude-plugins-official review
yaca plugins install name@marketplace
yaca plugins update
yaca plugins uninstall name@marketplace
```

The marketplace list includes the official `anthropics/claude-plugins-official` by default, alongside marketplaces known to Claude Code (`~/.claude/plugins/known_marketplaces.json`). Custom marketplaces can be added as GitHub `owner/repo`, git URL, or local directory; local directories are read in place. Nothing is downloaded until a marketplace is browsed or "Update" is clicked, at which point Yaca clones it into its `marketplaces` directory (without using Claude Code's clone). Built-in or Claude Code-imported marketplaces can be removed to hide them.

A plugin from a marketplace is fetched from a subfolder within the marketplace (`./path`), from its own repository (`url`, `github` with `ref`, `sha`, `path`), or from a subdirectory of another repository (`git-subdir`). Entries with `strict: false` or plugins without their own manifest use the catalog entry as their manifest. Sources like `npm`, `pip`, etc. are shown as unavailable.

Plugins without a marketplace (common for Agent Plugins) can be installed by address: `yaca plugins install <dir|owner/repo|git-url>`, named `plugin@direct`.

Installation details:

- Clones a single commit via git without history. Git is run non-interactively: if authentication is required, installation fails with a git error rather than hanging.
- Downloaded content is first validated using the same loader used in sessions. Plugins that fail validation are not installed, leaving no leftover files.
- Downloaded code is never run automatically: no `npm install`, no scripts. Plugin components (hooks, MCP servers) only run within a chat session under the same rules as manually installed plugins.
- Installed plugins are enabled by default: installation implies consent, matching Claude Code. The number of hooks and servers is shown directly in the list.
- Updates are manual only. Updating re-fetches the plugin and replaces files if changed; a new marketplace commit without changes in the plugin itself is not treated as an update.
- Uninstallation removes plugin files, the installation record, and the toggle. The plugin's `plugin-data` folder is preserved to protect user data.

Not supported in installation:

- **`npm` and `pip` sources**, or marketplaces provided as a standalone JSON URL.
- **Auto-updates** or update checks on startup.
- **Signature verification** and publisher trust: trust is based on the source repository URL.
- **Marketplace `renames`**: renamed plugins appear under the new name, while an older installation continues updating under the old name as long as that entry exists.
- **Clone progress bars**: the UI indicates background activity without percentage completion.

## MCP Servers

Servers can be defined in one of three locations, where more specific scopes completely override servers of the same name from broader scopes:

1. `mcp.json` in the app settings directory — user-wide servers for all projects;
2. `.mcp.json` in the project root — shared configuration used by other tools;
3. `mcpServers` in the project's `.yaca/settings.json`.

The configuration syntax is identical everywhere:

```json
{
  "mcpServers": {
    "files": { "command": "npx", "args": ["-y", "@example/server-files", "."] },
    "tracker": {
      "url": "https://tracker.example.com/mcp",
      "headers": { "Authorization": "Bearer ${TRACKER_TOKEN}" }
    }
  }
}
```

`command` launches a local server and communicates over stdio; `url` connects via Streamable HTTP. `${NAME}` and `${NAME:-default}` substitute environment variables so tokens don't need to be hardcoded in files. Optional fields include `env`, `cwd`, `disabled`, `startupTimeoutSecs` (default: 30), and `toolTimeoutSecs` (default: 600).

Server tools are presented to the model as `mcp__<server>__<tool>`. Tools marked as read-only by the server (`readOnlyHint`) run without confirmation prompts; others require confirmation similarly to network access. Servers defined in `.mcp.json` do not run until explicitly approved: the configuration file arrives with the repository, and stdio servers execute shell commands. Approval is granted in the `/mcp` panel or via `yaca mcp --approve <name>`, and persists until the definition changes.

The server list is loaded upon opening a chat: modifications take effect in subsequent chats, while an active chat maintains its initial server set.

### OAuth Login

A remote server that answers `401` is marked "login needed" in the `/mcp` panel, with a **Log in** button (from a terminal: `yaca mcp --login <name>`). Yaca follows the protocol's authorization flow for a public client: it finds the authorization server through the server's metadata (RFC 9728, RFC 8414, or the specification's defaults), registers itself as a client (RFC 7591), and opens the login page in the browser with PKCE; the answer comes back to a one-time listener on `127.0.0.1`. The tokens are kept in the system keychain by the server's address and refreshed when they expire, and the server connects again on its own. **Log out** forgets them. A server whose declaration sets its own `Authorization` header is never sent a login.

### Prompts and Forms

- **Prompts.** A server's prompt templates are slash commands named `/mcp__<server>__<prompt>`. Words after the name fill the prompt's arguments in the order the server declares them, the last taking the rest of the line. The model gets the filled prompt; the feed keeps what was typed.
- **Forms (elicitation).** A server may ask the human for input in the middle of its work. The form appears in the feed under the server's name, with its fields (text, number, yes/no, a choice), and **Send**, **Decline** or `Esc` answer it. An answer that does not fit the form is refused and the form stays open; a form nobody answers in 10 minutes is cancelled. In the CLI every form is declined.

Not supported in the MCP client:

- **Client registration without RFC 7591.** A server whose authorization server offers no dynamic registration cannot be logged in to; use `headers` with a token instead.
- **Legacy SSE transport** (two endpoints instead of one). Such servers are marked as incompatible.
- **Sampling**: the client does not advertise it and servers do not request it.
- **Resource subscriptions.** Resources are read on-demand by the model; servers do not push change notifications.
- **Progress notifications** during long-running tool calls: the feed displays the result once completed.

## Browser

The globe left of the panel button opens the browser column: a page of its own for each chat, between the feed and the right panel. The model works in it through an MCP server the app provides itself, `browser` (source "app" in the `/mcp` panel), so its calls get the same rows in the feed and the same permission questions as any other server's. The server runs inside the app and opens no port.

| Tool | What it does | Asks first |
|---|---|---|
| `navigate` | Opens a URL, or goes back, forward, or reloads. A bare `localhost:5173` opens over http. | yes |
| `screenshot` | The visible part, the whole page (up to three viewports down), or one element. In CSS pixels. | no |
| `read_page` | A snapshot of headings, text and interactive elements with refs (`e12`) for `act`, or the readable content as Markdown. | no |
| `act` | Click, double or right click, hover, type, press a key, scroll, choose in a `<select>`, wait for text or an element. Real input events. | yes |
| `eval_js` | Runs JavaScript in the page and returns the value as JSON; promises are awaited. | yes |
| `logs` | The page's console messages or network requests since it loaded. | no |
| `viewport` | Lays the page out at a preset (desktop, tablet, mobile) or a size, or back at the column's size. | no |

In Auto mode the reviewer model decides whether a call that asks first may run; it sees only the call, which is why `act` carries `element`, the model's own words for what it acts on. Each result begins with the page's URL and the size it is laid out at: the human may have clicked somewhere since the last call. While the column is open the page is the column's size; while it is closed the page keeps living out of sight at 1280×800, and screenshots go on working. A size set with `viewport` holds either way, and the column shows a larger page scaled down.

Screenshots show in their row of the feed, and a click opens them in the image viewer. Like every image an MCP server returns, they are kept under the project's `.yaca/pasted`, named by their content.

The human uses the same page: back, forward, reload, an address field, opening the page in the system browser, and DevTools. A teal line under the column's header runs while a call is in progress, the element the model acts on is ringed for a moment, and a dot on the globe shows that the model is using a page the human cannot see. The first page the model opens in a chat brings the column up. All chats share one profile (`browser\` in the app directory), separate from the window's own; pages cannot reach the app's commands, and the window's own pages cannot be opened in it.

Not supported:

- **macOS and Linux.** The engine is WebView2 driven over the DevTools Protocol. The tools are written against an engine interface (`src-tauri/src/browser/engine.rs`); WKWebView and WebKitGTK need implementations of it, and until then those builds show no globe.
- **Tabs.** One page per chat; links that open a new window open in the same page.
- **Downloads and uploads.** Downloads are blocked and reported to the model; file inputs cannot be filled.
- **Permission per site.** "Always allow" covers a tool, not a site.
- **Prompt dialogs.** `alert` and `confirm` are accepted on their own, `prompt` is dismissed, and the next result says so.
- **The CLI.** It has no window, and so no browser.

## PDFs and Notebooks

`Read` handles two kinds of files that are not plain text:

- **PDF.** The text is extracted page by page, in pure Rust, so nothing has to be installed next to the app, and it works with every provider, local models included. A document of up to 10 pages is read whole; a longer one needs `pages` (`"3"`, `"3-7"`, `"12-"`), at most 20 pages per call. A page without a text layer is marked as a scan rather than shown as blank.
- **Jupyter notebook (`.ipynb`).** Cells come back as a person sees them: number, type, id, source, and text outputs, with tracebacks stripped of terminal colours. `offset` and `limit` count cells instead of lines.

`NotebookEdit` changes one cell at a time: `replace` its source (optionally switching between code and markdown), `insert` a new cell after a named one or at the top, or `delete` one. Cells are named by `cell_id`, or by number in notebooks without ids. The tool writes the JSON itself, keeping the file's indentation, trailing newline, and the metadata of every untouched cell; replacing a code cell clears its outputs and execution count, since they belong to the old code. It follows the same rules as `Edit`: the notebook must have been read, must not have changed since, and the change shows up as a diff and in the file panel.

Not supported:

- **Scanned PDFs (OCR), images and figures inside PDFs,** and password-protected PDFs.
- **Image and HTML outputs of notebook cells.** Only their type is shown (`[image/png output omitted]`).
- **Running notebook cells.** Use `Bash` with `jupyter nbconvert --execute` or similar.

## Token Savings

Three tools prevent the model from echoing large blocks of text that already exist elsewhere:

- **`SendText`** presents a file or a line range to the user as a dedicated feed block, complete with path and line numbers above the text. The model receives a single line confirming that the text was displayed, without routing the text payload through its own output tokens. The block preserves the displayed snapshot: reopening the session shows the original content even if the file has changed on disk.
- **`SearchMessage`** searches across messages in the current conversation (user prompts and agent replies) and retrieves any of them verbatim by message number. It reads the session log directly, finding messages even after context compaction has purged them from the active window. Message numbers remain stable for the lifetime of the session.
- **`ExtractMessage`** dumps a message to `.yaca/drafts/<session>/<number>.md`. The text can then be modified using standard `Edit` and displayed via `SendText`: the model only pays for modified lines rather than regenerating the entire body.

`SearchMessage` and `ExtractMessage` are only available in sessions with disk logging enabled; they are not offered in ephemeral headless runs. None of the three tools are provided to subagents.

Not supported:

- **Output of foreground commands via `SendText`.** A background command's output is shown by its id (`command`), from the file its whole output goes to or from its buffer; a command that ran in the foreground has no output kept to show.
- **Searching tool outputs or other sessions** via `SearchMessage`.
- **Text displayed via `SendText` in session exports.** Exports are generated from the message log, recording only the invocation itself.

Drafts in `.yaca/drafts/<session>` are deleted with their chat, and drafts of chats that no longer exist are removed when a chat opens.

## Language Servers (LSP)

The model navigates code beyond just `grep` and `Read`: if a language server is available on the system, it queries genuine definitions, references, and types using the `Lsp` tool:

| `action` | Response |
|---|---|
| `definition` | Location where the identifier at the given position is defined |
| `references` | All usages of the identifier |
| `hover` | Type information and documentation |
| `symbols` | Outline/structure of a single file |
| `workspace_symbols` | Project-wide symbol search by name |
| `diagnostics` | Server errors and warnings for a file |

The position is specified as `path` + `line` + `symbol` (identifying substring within the line): the core calculates the exact column offset since models handle column counting poorly. The tool is strictly read-only and requires no permissions.

**Renaming.** The `Rename` tool renames a symbol everywhere it is used: it takes the same position (`path` + `line` + `symbol`) and a `new_name`, asks the server for `textDocument/rename`, and applies every edit the server returns. Unlike a text replace, it changes only the uses that are the same symbol. The rename is all-or-nothing: every file is read and every edit placed before anything is written, and an edit that does not fit its file stops the whole rename. It asks for permission like an edit of the named file, noting that other files may change too. Files the server names outside the project, guarded files, and files in additional roots (which a rollback would not restore) refuse the rename. The feed shows one row for the rename with a row per changed file under it, each with its own diff.

In addition to explicit queries, errors reported by the language server for newly written files are automatically appended to `Edit`, `Write`, and `Rename` results, allowing the model to immediately catch syntax or type errors it introduced. This only applies if a server for that file is already running; file edits do not start the server on their own.

**Server Discovery.** Built-in defaults: `rust-analyzer`, `typescript-language-server` (TS and JS), `pyright-langserver` (Python), and `gopls`. A default server is used only if its binary is found in `PATH`. Custom servers are configured in `lsp.json` in the app settings directory, alongside `mcp.json`:

```json
{
  "lspServers": {
    "clangd": { "command": "clangd", "extensions": ["c", "h", "cpp"] },
    "gopls": { "disabled": true }
  }
}
```

A matching entry replaces the built-in configuration completely; `disabled` deactivates the server. Servers start lazily on the first query targeting a file with a matching extension, rather than at chat startup. A crashed server triggers a "Language server not responding" warning banner, and subsequent queries attempt to relaunch it.

Unlike MCP, projects cannot define LSP servers in their configuration: a stdio server executes arbitrary commands, and a cloned repository must not run unauthorized executables on the user's machine. Consequently, `.yaca/settings.json` has no `lspServers` section.

Diagnostics come from the server's own publications (`publishDiagnostics`), or, from a server that declares pull diagnostics (`diagnosticProvider`), by asking it (`textDocument/diagnostic`) — some servers only answer when asked.

**The `/lsp` panel** lists the chat's language servers with their state, the files each handles, and the reason a failed one gave; **Restart** stops a server and starts it again. A failed server's warning bar opens the panel.

Not supported in LSP integration:

- **Code actions, autocomplete, or formatting.** Only navigation, diagnostics, and renaming.
- **Renames that move files.** The client tells servers it cannot apply file operations; if a server still asks to create, rename, or delete a file as part of a rename, the whole rename is refused, and the model is told to rename the symbol with `Edit` and the file with `Move`.
- **Filesystem watching.** Files modified outside Yaca are only re-inspected when the model queries them next.
- **Multiple servers per language** or multiple workspace roots: the server is bound to the primary project root.
- **Remote servers.** Only local processes communicating over stdio.

## Model Router

The router picks the model that answers a message. It is a provider of its own (`router`) made of the providers in `providers.json`, so the session sees one virtual model and nothing else changes. In the window it appears in the model switcher under "Router" once `routing.json` is valid, and is set up at the bottom of Settings → Providers; from the command line pick it with `--provider router`.

**How it works.** Per user message, a *decision model* — any model from `providers.json`, usually a small fast one — answers the typed questions (*signals*) you wrote in plain language, in one call. *Decisions* are rules over those answers; the highest-priority one that matches names the model. If nothing matches, or the questions time out, the message goes to `default`. The choice is made once per message and held for the whole tool loop: a thinking signature and a prompt cache are bound to their model, so switching mid-turn would break both.

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

- **Questions.** `noul` (yes/no, answer is P(yes)) and `choice` (one of several options). Without a `predicate`, a `noul` signal needs P(yes) ≥ 0.5 and a `choice` label must be the most probable option.
- **Rules.** `all`, `any`, `not` and `signal`, in three-valued logic: a signal the decision model did not answer is *unknown*, and `not(unknown)` stays unknown. What an unknown rule means is the decision's `onUnknown` (`noMatch` by default, so an unanswered question never sends a conversation to an expensive model).
- **Failures.** A decision model that is late, refuses, or answers something unreadable yields the default route; it never fails the turn. The feed shows a "Route: … → model" line once per message, and the CLI prints `Route: … → provider/model`.
- **A broken file.** It is never silently hidden: the switcher says why the router is missing, and Settings → Providers shows the same error the start would give.
- **Cost.** A routed request is priced by the model that answered, not by the router's entry.

A file that does not parse, or points at a provider or model that is not in `providers.json`, stops the start with a message naming the offending entry; it does not fall back to "no routing".

### Asking the Decision model

The same small model is available to the agent as a tool, `Decision`: one `question` in, a short answer out. The agent decides when to use it — typically to choose one option from a list (a skill, a file, an approach) without spending its own reasoning on it. There are no templates and no fixed question types: the question is passed as written, and the agent is told its answer is advice. The model it asks has no access to the conversation, the files or the tools, so the question has to carry everything it needs.

The tool exists only when there is a model to ask, and is not in the request at all otherwise. The model is the one chosen for the router in `routing.json` (`decisionModel`) when that file is valid; without a router, the first model the **Decision** provider answered with. If the model refuses or does not answer within 30 seconds the agent is told to decide for itself and carry on.

Not supported in the router:

- **`score`, `set` and `span` questions** and the selection algorithm where the decision model itself picks among candidate models. Only `noul` and `choice` with rule-based routing.
- **Real probabilities.** The decision model reports its own confidence in its JSON answer; token log-probabilities are not read, so on a weak model the numbers are coarse.
- **A form for every rule.** Settings → Providers has, under the list, a quick setup — pick the model that answers the questions, the default model and a strong one, and the editor is filled with one question ("does this need multi-step reasoning?") routing to the strong model — and below it a text editor that checks the file with the core's rules before writing it. Your own questions, several rules and `all`/`any`/`not` conditions are written in the editor; there is no structured form for them yet.
- **Switching to or from the router in an open chat.** A chat's provider is fixed when it opens, so the router is picked for a new chat, and an open router chat keeps it.
- **A stable decision across compaction.** If history is compacted in the middle of a tool loop, the router decides again (and drops reasoning made by another model); the turn may continue on a different model.
- **A running decision model.** It is not started or checked for you: a local model must already be served.

## Server Mode

Yaca can run its core on one machine and be used from another. `yaca-server` is the host without a window: it holds the chats, runs the tools and keeps the keys, and a Yaca window — on a desktop, or the Android app — is only a view onto it. On a desktop the window can still work locally; "Settings → Connection" switches it to a server and back.

```bash
scripts/install-server.sh root@your-server --domain yaca.example.com --email you@example.com
```

The script (Debian and Ubuntu) connects over SSH and sets up everything: a `yaca` user, `yaca-server` as a systemd service on `127.0.0.1:7878`, nginx in front of it with a Let's Encrypt certificate, and a token for your first device. It prints one line:

```text
yaca://connect?host=yaca.example.com&token=yct_…
```

Paste it into the address field of the app and both fields fill in. The domain must already point at the server. Without a domain use `--insecure` for plain HTTP, only inside a VPN or a trusted network. To skip building on a small server, build `yaca-server` yourself and pass `--binary path`.

Devices are told apart by their tokens:

```bash
sudo -u yaca yaca-server token add tablet    # prints the token once
sudo -u yaca yaca-server token list
sudo -u yaca yaca-server token revoke tablet # the device is cut off at its next ping
```

How it behaves:

- **Chats live on the server.** A dropped connection loses nothing: the window reconnects and takes the chats back from the journal, the way a reloaded desktop window does.
- **One device at a time.** A device that connects takes over the streams; the one it replaced is told so on screen and can take them back.
- **Projects are folders of the server.** Choosing a project opens a folder browser over the server's disk. Dragging files in is off against a server, and `＋` attaches pictures from the device, which are sent and kept in the project like a pasted one.
- **Keys stay on the server.** With no system keychain there (a server has none) they are kept in `secrets.json` in the settings directory, readable by its owner only — the protection an SSH key has. The settings screen calls this "keychain".
- **Tokens** are stored on the server as SHA-256 hashes (`server.json`), and on the device in the app's private storage.

Not supported yet:

- **A phone layout.** The Android app is the desktop interface in a smaller window; it is usable but not shaped for a touch screen.
- **The browser column** against a server. It belongs to the desktop shell, and the server has no window to draw it in.
- **Notifications** when the app is in the background: toasts are a desktop shell feature.
- **Several devices at once**, and a `yaca-server` binary in the releases: build it with `cargo build --release -p yaca-server`.
- The Android app has not been run on a device; only the shell's compilation for `aarch64-linux-android` and the server through a socket are tested.

## Architecture

```
crates/yaca-core/    core: agent loop, tools, permissions, checkpoints, model router
crates/yaca-host/    the commands and the event bridge between the core and a shell
crates/yaca-server/  the host behind a WebSocket, for a window on another machine
crates/yaca-cli/     headless CLI interface to the core
crates/yaca-evals/   agent evaluation benchmarks
src-tauri/           Tauri shell: the desktop window, and the phone's bare webview
src/                 React and TypeScript UI
scripts/             installer art, and the server installer
```

The core is completely decoupled from the GUI: it consumes commands and emits an event stream, and the frontend is purely a reactive projection of this stream. Between the two sits the host, which has no window in it either: the desktop shell calls it in-process and `yaca-server` calls it for a remote window, so both reach the same commands under the same names.

## Development

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
npm test
npx tauri build --bundles nsis   # Windows installer in target/release/bundle/nsis/
```

Pushing a `v*` tag (it must equal the version in `src-tauri/tauri.conf.json`) runs `.github/workflows/release.yml`, which builds the installer and creates a draft pre-release.

Contribution and coding guidelines are in [CLAUDE.md](CLAUDE.md); open design questions and roadmap items are in [TODO.md](TODO.md).

## License

MIT
