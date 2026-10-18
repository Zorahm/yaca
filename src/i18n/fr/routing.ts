/**
 * French strings: the model router — the settings section, the feed note and the
 * switcher's notice.
 */

import type { routing as source } from "../ru/routing";

export const routing = {
  "routing.title": "Routeur",
  "routing.lede":
    "Un petit modèle répond à des questions sur chaque message, et des règles choisissent quel modèle répond. Le fichier routing.json se trouve à côté de providers.json.",
  "routing.state_absent": "Non configuré.",
  "routing.state_ready":
    "Connecté : il apparaît dans la liste des modèles sous le nom « {name} ».",
  "routing.state_broken": "Non connecté : {error}",
  "routing.editor": "Contenu de routing.json",
  "routing.save": "Enregistrer",
  "routing.remove": "Supprimer le routeur",
  "routing.saved": "Enregistré.",
  "routing.next_chat":
    "Les conversations déjà ouvertes continuent comme avant ; une nouvelle conversation prend le routeur de ce fichier.",
  "routing.default_route": "par défaut",
  "routing.chip": "Route : {route} → {model}",
  "routing.menu_broken": "Le routeur n’est pas connecté : {error}",
  "routing.wizard_title": "Configuration rapide",
  "routing.wizard_lede":
    "Choisissez trois modèles et l’éditeur se remplit d’une configuration prête : le modèle puissant prend les demandes difficiles, tout le reste va au modèle par défaut. Rien n’est écrit tant que vous n’enregistrez pas.",
  "routing.wizard_decider": "Modèle qui répond aux questions",
  "routing.wizard_default": "Modèle par défaut",
  "routing.wizard_strong": "Modèle puissant pour le difficile",
  "routing.wizard_build": "Remplir l’éditeur",
  "routing.wizard_none": "Aucun modèle : connectez d’abord un fournisseur.",
  "routing.wizard_pick": "Choisir un modèle",
  "routing.decision_hint":
    "Le modèle de décision se prend de préférence chez le fournisseur « Decision » ci-dessus : indiquez son adresse, locale ou cloud, et interrogez ses modèles.",
} satisfies Record<keyof typeof source, string>;
