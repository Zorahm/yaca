/**
 * German strings: the model router — the settings section, the feed note and the
 * switcher's notice.
 */

import type { routing as source } from "../ru/routing";

export const routing = {
  "routing.title": "Router",
  "routing.lede":
    "Ein kleines Modell beantwortet Fragen zu jeder Nachricht, und Regeln bestimmen, welches Modell antwortet. Die Datei routing.json liegt neben providers.json.",
  "routing.state_absent": "Nicht eingerichtet.",
  "routing.state_ready": "Verbunden: In der Modellliste heißt er „{name}“.",
  "routing.state_broken": "Nicht verbunden: {error}",
  "routing.editor": "Inhalt von routing.json",
  "routing.save": "Speichern",
  "routing.remove": "Router entfernen",
  "routing.saved": "Gespeichert.",
  "routing.next_chat":
    "Bereits geöffnete Chats arbeiten wie bisher weiter; ein neuer Chat übernimmt den Router aus dieser Datei.",
  "routing.default_route": "Standard",
  "routing.chip": "Route: {route} → {model}",
  "routing.menu_broken": "Der Router ist nicht verbunden: {error}",
  "routing.wizard_title": "Schnelleinrichtung",
  "routing.wizard_lede":
    "Wählen Sie drei Modelle, und der Editor füllt sich mit einer fertigen Einrichtung: Das starke Modell übernimmt schwierige Anfragen, alles andere geht an das Standardmodell. Gespeichert wird erst, wenn Sie speichern.",
  "routing.wizard_decider": "Modell, das die Fragen beantwortet",
  "routing.wizard_default": "Standardmodell",
  "routing.wizard_strong": "Starkes Modell für Schwieriges",
  "routing.wizard_build": "Editor füllen",
  "routing.wizard_none": "Noch keine Modelle: Verbinden Sie zuerst einen Anbieter.",
  "routing.wizard_pick": "Modell wählen",
  "routing.decision_hint":
    "Das Entscheidungsmodell nimmt man am besten vom Anbieter „Decision“ oben: Adresse eintragen, lokal oder in der Cloud, und die Modelle abfragen.",
} satisfies Record<keyof typeof source, string>;
