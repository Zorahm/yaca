/**
 * English strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen.
 */

import type { shell as source } from "../ru/shell";

export const shell = {
  "side.new": "New chat",
  "side.projects": "Projects",
  "side.new_here": "New chat in this project",
  "side.toggle": "Sidebar",
  "side.update_do": "Update",
  "side.update_more": "Details",
  "side.forget": "Remove project",
  "side.forget_busy": "Close this project's chats first",
  "side.no_projects": "No projects yet",
  "side.working": "Working",
  "side.waiting": "Waiting for an answer",
  "side.close_chat": "Close chat",

  "top.menu": "Application menu",
  "top.sessions": "Sessions",
  "top.search": "Search this session",
  "top.back": "Back",
  "top.forward": "Forward",
  "top.processes": "Processes",
  "top.checkpoints": "Checkpoints",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "Right panel",
  "panel.resize": "Panel width",
  "panel.tab_agents": "Agents",
  "side.resize": "Sidebar width",
  "top.session_menu": "Session menu",
  "top.untitled": "New session",
  "top.rename_hint": "Double-click to rename",

  "home.greeting": "Where do we start?",
  "home.greeting_named": "Where do we start, {name}?",
  "home.pick_project": "Create project",
  "home.pick_title": "Project",
  "home.not_chosen": "not chosen",
  "home.lead":
    "Choose a project in the sidebar or create one with related folders. Mode, model and effort can be changed before the first message.",
   "home.lead_chat":
     "Write what you need, and the agent starts the conversation. Mode, model and effort can be changed before the first message.",
  "home.no_provider": "No provider.",
  "home.no_provider_hint": "Add a model key to be able to write messages.",

  "title.minimize": "Minimize",
  "title.maximize": "Maximize",
  "title.restore": "Restore",
  "title.close": "Hide to system tray",
  "tray.open": "Open Yaca",
  "menu.help": "Help",
  "menu.check_updates": "Check for updates",
  "menu.about": "About Yaca",

  "common.open_project": "Open a project",
  "common.close": "Close",
  "common.seconds": "{n}s",
} satisfies Record<keyof typeof source, string>;
