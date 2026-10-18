/**
 * French strings: the browser column.
 */

import type { browser as source } from "../ru/browser";

export const browser = {
  "browser.toggle": "Navigateur",
  "browser.back": "Précédent",
  "browser.forward": "Suivant",
  "browser.reload": "Recharger",
  "browser.stop": "Arrêter le chargement",
  "browser.address": "Adresse ou localhost:port",
  "browser.external": "Ouvrir dans le navigateur du système",
  "browser.devtools": "Outils de développement",
  "browser.empty_title": "Rien d'ouvert pour l'instant",
  "browser.empty_body": "La page que le modèle ou vous ouvrirez apparaîtra ici",
  "browser.size_model": "{size} · fixé par le modèle",
  "browser.reset_size": "Revenir à la taille de la colonne",
  "browser.resize": "Redimensionner le navigateur",
  "browser.bad_address": "Cela ne ressemble pas à une adresse de page",
  "browser.open_failed": "La page ne s'est pas ouverte",
  "browser.busy": "Le modèle utilise le navigateur",
} satisfies Record<keyof typeof source, string>;
