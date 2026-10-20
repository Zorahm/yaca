/**
 * German strings: connecting to a server.
 */

import type { link as source } from "../ru/link";

export const link = {
  "link.title": "Mit einem Yaca-Server verbinden",
  "link.intro": "Yaca läuft auf Ihrem Server, diese App ist nur ein Fenster dorthin. Geben Sie die Adresse und den Token ein, die das Installationsskript ausgegeben hat.",
  "link.address": "Serveradresse",
  "link.address_hint": "Zum Beispiel yaca.example.com",
  "link.token": "Token",
  "link.token_hint": "Wird von yaca-server token add ausgegeben.",
  "link.connect": "Verbinden",
  "link.connecting": "Verbinde …",
  "link.local": "Auf diesem Computer arbeiten",
  "link.err_unreachable": "Der Server antwortet nicht. Prüfen Sie Adresse und Netzwerk.",
  "link.err_unauthorized": "Der Server hat den Token nicht akzeptiert.",
  "link.err_bad_address": "Das sieht nicht nach einer Serveradresse aus.",
  "link.err_replaced": "Ein anderes Gerät hat sich mit dem Server verbunden und die Chats übernommen. Verbinden Sie sich erneut, um sie hierher zurückzuholen.",
  "link.err_revoked": "Der Token dieses Geräts wurde widerrufen.",
  "link.lost": "Verbindung zum Server verloren. Verbinde erneut …",
  "link.settings_nav": "Verbindung",
  "link.settings_title": "Verbindung",
  "link.settings_this_pc": "Die Arbeit läuft auf diesem Computer.",
  "link.settings_server": "Die Arbeit läuft auf dem Server {name}.",
  "link.settings_to_server": "Mit einem Server verbinden",
  "link.settings_to_local": "Zurück zu diesem Computer",
  "link.settings_disconnect": "Vom Server trennen",
  "link.settings_reload_note": "Das Fenster wird neu geladen. Die Chats auf dem Server laufen weiter.",
  "link.pick_up": "Nach oben",
  "link.pick_home": "Home",
  "link.pick_choose": "Diesen Ordner wählen",
  "link.pick_new": "Neuer Ordner",
  "link.pick_new_name": "Ordnername",
  "link.pick_create": "Erstellen",
  "link.pick_cancel": "Abbrechen",
  "link.pick_empty": "Keine Unterordner",
  "link.pick_hidden": "Versteckte Ordner",
  "link.pick_broad": "Das sieht nach einem Home- oder Stammordner aus, nicht nach einem Projekt. Der Agent würde mit allem darin arbeiten, und Schnappschüsse zum Zurücksetzen funktionieren nicht.",
  "link.pick_loading": "Lese den Ordner …",
} satisfies Record<keyof typeof source, string>;
