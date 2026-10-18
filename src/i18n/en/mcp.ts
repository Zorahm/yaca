/**
 * English strings: MCP servers — the settings section and the panel.
 */

import type { mcp as source } from "../ru/mcp";

export const mcp = {
  "mcp.title": "MCP servers",
  "mcp.lede_app":
    "Your own servers, connected in every project. The agent gets their tools beside the built-in ones.",
  "mcp.lede_project":
    "This project's servers. Kept in .yaca/settings.json, and replacing a server of the same name from your own list and from .mcp.json.",
  "mcp.add": "Add a server",
  "mcp.none_declared": "No servers.",
  "mcp.edit": "Edit",
  "mcp.enabled": "Connect {name}",
  "mcp.delete": "Delete",
  "mcp.delete_confirm": "Delete?",
  "mcp.file_title": "From .mcp.json",
  "mcp.file_hint":
    "The project's file, which other tools read too. Only shown here. Its servers start once approved in a chat: /mcp.",
  "mcp.file_replaced": "replaced by the server of the same name above",
  "mcp.next_chat_note":
    "The list is read when a chat opens. A change reaches the next chat; an open one keeps the servers it started with.",
  "mcp.form_new": "New server",
  "mcp.form_save": "Save the server",
  "mcp.form_name": "Name",
  "mcp.form_name_hint":
    "Part of the name of every tool the server has. Latin letters, digits, hyphen and underscore.",
  "mcp.form_kind": "How to connect",
  "mcp.kind_stdio": "Program",
  "mcp.kind_http": "Address",
  "mcp.form_command": "Program",
  "mcp.form_command_hint": "Started directly, not through a shell.",
  "mcp.form_args": "Arguments",
  "mcp.form_args_hint": "One per line.",
  "mcp.form_env": "Environment",
  "mcp.form_env_hint":
    "NAME=value, one per line. A value may refer to an environment variable, as in the example: that keeps a token out of the file.",
  "mcp.form_cwd": "Working directory",
  "mcp.form_cwd_hint": "Empty means the project root.",
  "mcp.form_url": "Address",
  "mcp.form_url_hint":
    "A server that speaks Streamable HTTP. Signing in through OAuth is not supported: a token goes in a header.",
  "mcp.form_headers": "Headers",
  "mcp.form_headers_hint":
    "Name: value, one per line. A value may refer to an environment variable, as in the example.",
  "mcp.problem_name_empty": "Give it a name.",
  "mcp.problem_name_chars": "A name may hold Latin letters, digits, hyphen and underscore.",
  "mcp.problem_name_taken": "There is already a server with this name.",
  "mcp.problem_command_empty": "Give the program.",
  "mcp.problem_env": "Every environment line is NAME=value.",
  "mcp.problem_url": "The address must start with http:// or https://.",
  "mcp.problem_headers": "Every header line is Name: value.",

  "mcp.panel_empty":
    "This chat has no MCP servers. Declare them in the settings and they will be there in the next chat.",
  "mcp.status_connecting": "connecting",
  "mcp.status_ready": "ready",
  "mcp.status_failed": "failed",
  "mcp.status_disabled": "off",
  "mcp.status_needs_approval": "waiting for approval",
  "mcp.source_global": "your own list",
  "mcp.source_project_file": "the project's .mcp.json",
  "mcp.source_project": "project settings",
  "mcp.source_plugin": "plugin",
  "mcp.source_app": "app",
  "mcp.approve": "Approve",
  "mcp.reconnect": "Reconnect",
  "mcp.status_needs_login": "login needed",
  "mcp.status_logging_in": "logging in",
  "mcp.login": "Log in",
  "mcp.logout": "Log out",
  "mcp.login_note": "Finish logging in in the browser; the server connects once you are back.",
  "mcp.login_open": "Open the login page",
  "lsp.panel_empty": "No language servers in this chat. Built-in ones start when their binary is on PATH; others are added in Settings.",
  "lsp.status_idle": "not started",
  "lsp.status_starting": "starting",
  "lsp.status_ready": "ready",
  "lsp.status_failed": "failed",
  "lsp.status_disabled": "off",
  "lsp.restart": "Restart",
  "lsp.start": "Start",
  "lsp.extensions": "Files: {list}",
  "mcp.approval_note":
    "Declared in the project's .mcp.json and not started yet. Approving starts the program above, and is remembered until the declaration changes.",
  "mcp.tools.one": "{n} tool",
  "mcp.tools.few": "{n} tools",
  "mcp.tools.many": "{n} tools",
  "mcp.resources": "resources",
  "mcp.tool_reads": "reads",
  "mcp.tool_writes": "writes",
} satisfies Record<keyof typeof source, string>;
