/**
 * German strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen.
 */

import type { shell as source } from "../ru/shell";

export const shell = {
  "side.new": "Neuer Chat",
  "side.projects": "Projekte",
  "side.new_here": "Neuer Chat in diesem Projekt",
  "side.toggle": "Seitenleiste",
  "side.update_do": "Aktualisieren",
  "side.update_more": "Details",
  "side.forget": "Projekt entfernen",
  "side.forget_busy": "Zuerst die Chats dieses Projekts schließen",
  "side.no_projects": "Noch keine Projekte",
  "side.working": "Arbeitet",
  "side.waiting": "Wartet auf Antwort",
  "side.close_chat": "Chat schließen",

  "top.menu": "Anwendungsmenü",
  "top.sessions": "Sitzungen",
  "top.search": "In dieser Sitzung suchen",
  "top.back": "Zurück",
  "top.forward": "Vorwärts",
  "top.processes": "Prozesse",
  "top.checkpoints": "Checkpoints",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "Rechtes Panel",
  "panel.resize": "Breite des Panels",
  "panel.tab_agents": "Agenten",
  "side.resize": "Breite der Seitenleiste",
  "top.session_menu": "Sitzungsmenü",
  "top.untitled": "Neue Sitzung",
  "top.rename_hint": "Doppelklick zum Umbenennen",

  "home.greeting": "Womit fangen wir an?",
  "home.greeting_named": "Womit fangen wir an, {name}?",
  "home.pick_project": "Projekt erstellen",
  "home.pick_title": "Projekt",
  "home.not_chosen": "nicht gewählt",
  "home.lead":
    "Wählen Sie ein Projekt in der Seitenleiste oder erstellen Sie eines mit verwandten Ordnern. Modus, Modell und Aufwand lassen sich vor der ersten Nachricht ändern.",
   "home.lead_chat":
     "Schreiben Sie, wobei Sie Hilfe brauchen, dann beginnt der Agent das Gespräch. Modus, Modell und Aufwand lassen sich bis zur ersten Nachricht ändern.",
  "home.no_provider": "Kein Anbieter.",
  "home.no_provider_hint": "Einen Modellschlüssel hinzufügen, um Nachrichten schreiben zu können.",

  "title.minimize": "Minimieren",
  "title.maximize": "Maximieren",
  "title.restore": "Wiederherstellen",
  "title.close": "In den Infobereich minimieren",
  "tray.open": "Yaca öffnen",
  "menu.help": "Hilfe",
  "menu.check_updates": "Nach Updates suchen",
  "menu.about": "Über Yaca",

  "common.open_project": "Projekt öffnen",
  "common.close": "Schließen",
  "common.seconds": "{n} s",
} satisfies Record<keyof typeof source, string>;
