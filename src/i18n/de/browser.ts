/**
 * German strings: the browser column.
 */

import type { browser as source } from "../ru/browser";

export const browser = {
  "browser.toggle": "Browser",
  "browser.back": "Zurück",
  "browser.forward": "Vorwärts",
  "browser.reload": "Neu laden",
  "browser.stop": "Laden stoppen",
  "browser.address": "Adresse oder localhost:Port",
  "browser.external": "Im Systembrowser öffnen",
  "browser.devtools": "Entwicklertools",
  "browser.empty_title": "Noch nichts geöffnet",
  "browser.empty_body": "Hier erscheint die Seite, die das Modell oder Sie öffnen",
  "browser.size_model": "{size} · vom Modell gesetzt",
  "browser.reset_size": "Zurück zur Spaltengröße",
  "browser.resize": "Browserbreite ändern",
  "browser.bad_address": "Das sieht nicht nach einer Seitenadresse aus",
  "browser.open_failed": "Die Seite wurde nicht geöffnet",
  "browser.busy": "Das Modell arbeitet im Browser",
} satisfies Record<keyof typeof source, string>;
