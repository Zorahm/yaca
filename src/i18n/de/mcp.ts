/**
 * German strings: MCP servers — the settings section and the panel.
 */

import type { mcp as source } from "../ru/mcp";

export const mcp = {
  "mcp.title": "MCP-Server",
  "mcp.lede_app":
    "Eigene Server, in jedem Projekt verbunden. Deren Tools erhält der Agent neben den eingebauten.",
  "mcp.lede_project":
    "Die Server dieses Projekts. Sie liegen in .yaca/settings.json und ersetzen gleichnamige aus der allgemeinen Liste und aus .mcp.json.",
  "mcp.add": "Server hinzufügen",
  "mcp.none_declared": "Keine Server.",
  "mcp.edit": "Bearbeiten",
  "mcp.enabled": "{name} verbinden",
  "mcp.delete": "Löschen",
  "mcp.delete_confirm": "Löschen?",
  "mcp.file_title": "Aus .mcp.json",
  "mcp.file_hint":
    "Die Datei des Projekts, die auch andere Tools lesen. Hier nur zur Ansicht. Ihre Server starten erst nach Freigabe im Chat: /mcp.",
  "mcp.file_replaced": "durch den gleichnamigen Server oben ersetzt",
  "mcp.next_chat_note":
    "Die Liste wird beim Öffnen eines Chats gelesen. Änderungen gelangen in den nächsten Chat; ein geöffneter behält die Server, mit denen er gestartet ist.",
  "mcp.form_new": "Neuer Server",
  "mcp.form_save": "Server speichern",
  "mcp.form_name": "Name",
  "mcp.form_name_hint":
    "Ist Teil des Namens jedes Tools des Servers. Lateinische Buchstaben, Ziffern, Bindestrich und Unterstrich.",
  "mcp.form_kind": "Verbindungsart",
  "mcp.kind_stdio": "Programm",
  "mcp.kind_http": "Adresse",
  "mcp.form_command": "Programm",
  "mcp.form_command_hint": "Wird direkt gestartet, nicht über eine Shell.",
  "mcp.form_args": "Argumente",
  "mcp.form_args_hint": "Eines pro Zeile.",
  "mcp.form_env": "Umgebung",
  "mcp.form_env_hint":
    "NAME=Wert, eine Zeile pro Eintrag. Ein Wert kann auf eine Umgebungsvariable verweisen, wie im Beispiel: So landet kein Token in der Datei.",
  "mcp.form_cwd": "Arbeitsverzeichnis",
  "mcp.form_cwd_hint": "Leer bedeutet Projektstamm.",
  "mcp.form_url": "Adresse",
  "mcp.form_url_hint":
    "Ein Server mit Streamable HTTP. Die Anmeldung über OAuth wird nicht unterstützt: Ein Token wird über einen Header übergeben.",
  "mcp.form_headers": "Header",
  "mcp.form_headers_hint":
    "Name: Wert, eine Zeile pro Eintrag. Ein Wert kann auf eine Umgebungsvariable verweisen, wie im Beispiel.",
  "mcp.problem_name_empty": "Namen angeben.",
  "mcp.problem_name_chars": "Im Namen sind lateinische Buchstaben, Ziffern, Bindestrich und Unterstrich erlaubt.",
  "mcp.problem_name_taken": "Ein Server mit diesem Namen existiert bereits.",
  "mcp.problem_command_empty": "Programm angeben.",
  "mcp.problem_env": "Jede Umgebungszeile hat die Form NAME=Wert.",
  "mcp.problem_url": "Die Adresse muss mit http:// oder https:// beginnen.",
  "mcp.problem_headers": "Jede Header-Zeile hat die Form Name: Wert.",

  "mcp.panel_empty":
    "Dieser Chat hat keine MCP-Server. Sie in den Einstellungen anlegen, dann sind sie im nächsten Chat dabei.",
  "mcp.status_connecting": "verbindet",
  "mcp.status_ready": "bereit",
  "mcp.status_failed": "Fehler",
  "mcp.status_disabled": "aus",
  "mcp.status_needs_approval": "wartet auf Freigabe",
  "mcp.source_global": "allgemeine Liste",
  "mcp.source_project_file": ".mcp.json des Projekts",
  "mcp.source_project": "Projekteinstellungen",
  "mcp.source_plugin": "Plugin",
  "mcp.source_app": "App",
  "mcp.approve": "Freigeben",
  "mcp.reconnect": "Neu verbinden",
  "mcp.status_needs_login": "Anmeldung nötig",
  "mcp.status_logging_in": "Anmeldung läuft",
  "mcp.login": "Anmelden",
  "mcp.logout": "Abmelden",
  "mcp.login_note": "Schließen Sie die Anmeldung im Browser ab; der Server verbindet sich danach.",
  "mcp.login_open": "Anmeldeseite öffnen",
  "lsp.panel_empty": "In diesem Chat gibt es keine Sprachserver. Eingebaute starten, wenn ihr Programm im PATH liegt; weitere werden in den Einstellungen hinzugefügt.",
  "lsp.status_idle": "nicht gestartet",
  "lsp.status_starting": "startet",
  "lsp.status_ready": "bereit",
  "lsp.status_failed": "fehlgeschlagen",
  "lsp.status_disabled": "aus",
  "lsp.restart": "Neu starten",
  "lsp.start": "Starten",
  "lsp.extensions": "Dateien: {list}",
  "mcp.approval_note":
    "Im .mcp.json des Projekts angemeldet und noch nicht gestartet. Die Freigabe startet das oben genannte Programm und bleibt gespeichert, bis sich die Deklaration ändert.",
  "mcp.tools.one": "{n} Tool",
  "mcp.tools.few": "{n} Tools",
  "mcp.tools.many": "{n} Tools",
  "mcp.resources": "Ressourcen",
  "mcp.tool_reads": "liest",
  "mcp.tool_writes": "schreibt",
} satisfies Record<keyof typeof source, string>;
