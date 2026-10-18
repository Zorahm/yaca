/**
 * French strings: providers — the settings section and the panel before it.
 */

import type { providers as source } from "../ru/providers";

export const providers = {
  // Providers: the panel that exists until the settings window.
  "prov.title": "Fournisseurs",
  "prov.home_button": "Configurer les fournisseurs",
  "prov.add": "Ajouter un fournisseur",
  "prov.edit_row": "Configurer",
  "prov.delete": "Supprimer",
  "prov.delete_confirm": "Supprimer définitivement ?",
  "prov.models_count.one": "{n} modèle",
  "prov.models_count.few": "{n} modèles",
  "prov.models_count.many": "{n} modèles",

  "prov.form_new": "Nouveau fournisseur",
  "prov.form_name": "Nom",
  "prov.form_more": "Avancé",
  "prov.form_protocol": "Protocole",
  "prov.form_base_url": "URL de base",
  "prov.form_base_url_hint":
    "Le chemin du protocole est ajouté automatiquement : /v1/chat/completions pour OpenAI, /v1/messages pour Anthropic. Pour OpenRouter, c'est https://openrouter.ai/api. Si l'adresse indique déjà une version (/v1, /v1beta, /v4), /v1 n'est pas ajouté.",
  "prov.form_headers": "En-têtes personnalisés",
  "prov.form_headers_hint":
    "Ils s'ajoutent aux en-têtes du protocole et peuvent les remplacer, y compris authorization. Un {key} dans une valeur reçoit la clé du trousseau ; la clé elle-même n'est jamais écrite dans le fichier. Une valeur vide supprime l'en-tête du protocole.",
  "prov.form_header_name": "Nom",
  "prov.form_header_value": "Valeur",
  "prov.form_add_header": "Ajouter un en-tête",
  "prov.form_remove_header": "Retirer l'en-tête",

  "prov.form_key_title": "Clé",
  "prov.form_key_hint":
    "Elle est enregistrée dans le trousseau système sous le nom du fournisseur — providers.json ne la contient jamais",
  "prov.form_key_new_hint":
    "Une fois enregistré, le fournisseur est interrogé lui-même : ce qu'il répond apparaît dans le choix de modèle",
  "prov.key_save": "Enregistrer la clé",
  "prov.key_forget": "Oublier la clé",
  "prov.key_source_keychain": "clé dans le trousseau système",
  "prov.key_source_environment": "clé dans une variable d'environnement",
  "prov.key_source_none": "aucune clé",
  "prov.key_source_unknown": "la source apparaîtra une fois le fournisseur enregistré",
  "prov.key_not_needed": "Pas de clé nécessaire : {key} n'apparaît dans aucun en-tête",

  "prov.form_models": "Modèles",
  "prov.form_models_hint":
    "La liste vient du fournisseur lui-même : il est interrogé dès qu'une clé est enregistrée, et « Interroger » pose la question à nouveau. Yaca travaille avec ce qu'il a répondu ; le prix est donné en dollars par million de tokens, zéro signifiant « gratuit ou inconnu ».",
  "prov.form_model_context": "Contexte, tokens",
  "prov.form_model_output": "Sortie max., tokens",
  "prov.form_model_effort": "gère l'effort",
  "prov.models_ask": "Interroger",
  "prov.models_asking": "Interrogation…",
  "prov.models_none": "Aucun modèle",
  "prov.models_price": "$ / 1M en entrée et en sortie",
  "prov.models_effort": "effort",

  "prov.form_save": "Enregistrer le fournisseur",
  "prov.form_cancel": "Annuler",

  "prov.saved_unasked":
    "« {name} » est enregistré, mais il n'a pas répondu avec ses modèles : {error}",
  "prov.error_name": "Donnez un nom au fournisseur",
  "prov.error_url": "Indiquez l'URL de base du fournisseur",
  "prov.error_header_name": "L'en-tête {n} n'a pas de nom",
  "prov.error_header_dup": "L'en-tête {name} est indiqué deux fois",

  "prov.lede": "Destination de la conversation et de la clé. Niveau de l'application : un projet ne décide pas de cela.",
  "prov.answered": "A répondu avec sa liste de modèles",
  "prov.unasked": "pas encore interrogé",
  "prov.unasked_note":
    "Un point vide signifie que le fournisseur n'a jamais répondu par une liste de modèles. Il n'apparaîtra pas dans le sélecteur de modèle : une session n'aurait rien pour envoyer sa requête.",
} satisfies Record<keyof typeof source, string>;
