/**
 * German strings: providers — the settings section and the panel before it.
 */

import type { providers as source } from "../ru/providers";

export const providers = {
  // Providers: the panel that exists until the settings window.
  "prov.title": "Anbieter",
  "prov.home_button": "Anbieter einrichten",
  "prov.add": "Anbieter hinzufügen",
  "prov.edit_row": "Einrichten",
  "prov.delete": "Löschen",
  "prov.delete_confirm": "Endgültig löschen?",
  "prov.models_count.one": "{n} Modell",
  "prov.models_count.few": "{n} Modelle",
  "prov.models_count.many": "{n} Modelle",

  "prov.form_new": "Neuer Anbieter",
  "prov.form_name": "Name",
  "prov.form_more": "Erweitert",
  "prov.form_protocol": "Protokoll",
  "prov.form_base_url": "Basis-URL",
  "prov.form_base_url_hint":
    "Der Protokollpfad wird automatisch ergänzt: /v1/chat/completions bei OpenAI, /v1/messages bei Anthropic. Für OpenRouter lautet er https://openrouter.ai/api. Steht eine Version bereits in der Adresse (/v1, /v1beta, /v4), wird kein /v1 angehängt.",
  "prov.form_headers": "Eigene Header",
  "prov.form_headers_hint":
    "Werden zu den Protokoll-Headern hinzugefügt und ersetzen sie – auch authorization. {key} im Wert erhält den Schlüssel aus dem Schlüsselbund; der Schlüssel selbst landet nie in der Datei. Ein leerer Wert entfernt den Protokoll-Header.",
  "prov.form_header_name": "Name",
  "prov.form_header_value": "Wert",
  "prov.form_add_header": "Header hinzufügen",
  "prov.form_remove_header": "Header entfernen",

  "prov.form_key_title": "Schlüssel",
  "prov.form_key_hint":
    "Wird unter dem Namen des Anbieters im Systemschlüsselbund gespeichert – in providers.json steht er nie",
  "prov.form_key_new_hint":
    "Nach dem Speichern wird der Anbieter selbst nach seinen Modellen gefragt: Was er antwortet, erscheint bei der Modellauswahl",
  "prov.key_save": "Schlüssel speichern",
  "prov.key_forget": "Schlüssel vergessen",
  "prov.key_source_keychain": "Schlüssel im Systemschlüsselbund",
  "prov.key_source_environment": "Schlüssel in einer Umgebungsvariablen",
  "prov.key_source_none": "kein Schlüssel",
  "prov.key_source_unknown": "Die Quelle wird sichtbar, sobald der Anbieter gespeichert ist",
  "prov.key_not_needed": "Kein Schlüssel nötig: {key} kommt in keinem Header vor",

  "prov.form_models": "Modelle",
  "prov.form_models_hint":
    "Die Liste kommt vom Anbieter selbst: Er wird abgefragt, sobald ein Schlüssel gespeichert ist, und „Abfragen“ fragt erneut. Was er antwortet, damit wird gearbeitet; der Preis steht in Dollar pro Million Token, null bedeutet „kostenlos oder unbekannt“.",
  "prov.form_model_context": "Kontext, Token",
  "prov.form_model_output": "Max. Ausgabe, Token",
  "prov.form_model_effort": "unterstützt Aufwand",
  "prov.models_ask": "Abfragen",
  "prov.models_asking": "Frage ab…",
  "prov.models_none": "Keine Modelle",
  "prov.models_price": "$ / 1 Mio. Ein- und Ausgabe",
  "prov.models_effort": "Aufwand",

  "prov.form_save": "Anbieter speichern",
  "prov.form_cancel": "Abbrechen",

  "prov.saved_unasked":
    "„{name}“ ist gespeichert, hat aber keine Modellliste geliefert: {error}",
  "prov.error_name": "Name angeben",
  "prov.error_url": "Basis-URL angeben",
  "prov.error_header_name": "Header {n} hat keinen Namen",
  "prov.error_header_dup": "Header {name} ist doppelt angegeben",

  "prov.lede": "Wohin Gespräch und Schlüssel gehen. Anwendungsebene: Ein Projekt entscheidet das nicht.",
  "prov.answered": "Hat mit seinen Modellen geantwortet",
  "prov.unasked": "noch nicht abgefragt",
  "prov.unasked_note":
    "Ein leerer Kreis bedeutet, dass der Anbieter nie eine Modellliste geliefert hat. Er erscheint nicht bei der Modellauswahl: Einer Sitzung fehlte dann alles, womit sie eine Anfrage senden könnte.",
} satisfies Record<keyof typeof source, string>;
