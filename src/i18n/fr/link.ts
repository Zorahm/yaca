/**
 * French strings: connecting to a server.
 */

import type { link as source } from "../ru/link";

export const link = {
  "link.title": "Se connecter à un serveur Yaca",
  "link.intro": "Yaca tourne sur votre serveur et cette application n'en est que la fenêtre. Saisissez l'adresse et le jeton donnés par le script d'installation.",
  "link.address": "Adresse du serveur",
  "link.address_hint": "Par exemple, yaca.example.com",
  "link.token": "Jeton",
  "link.token_hint": "Affiché par yaca-server token add.",
  "link.connect": "Se connecter",
  "link.connecting": "Connexion…",
  "link.local": "Travailler sur cet ordinateur",
  "link.err_unreachable": "Le serveur ne répond pas. Vérifiez l'adresse et le réseau.",
  "link.err_unauthorized": "Le serveur n'a pas accepté le jeton.",
  "link.err_bad_address": "Cela ne ressemble pas à une adresse de serveur.",
  "link.err_replaced": "Un autre appareil s'est connecté au serveur et a pris les conversations. Reconnectez-vous pour les rapatrier ici.",
  "link.err_revoked": "Le jeton de cet appareil a été révoqué.",
  "link.lost": "Connexion au serveur perdue. Reconnexion…",
  "link.settings_nav": "Connexion",
  "link.settings_title": "Connexion",
  "link.settings_this_pc": "Le travail se fait sur cet ordinateur.",
  "link.settings_server": "Le travail se fait sur le serveur {name}.",
  "link.settings_to_server": "Se connecter à un serveur",
  "link.settings_to_local": "Revenir à cet ordinateur",
  "link.settings_disconnect": "Se déconnecter du serveur",
  "link.settings_reload_note": "La fenêtre se recharge. Les conversations du serveur continuent.",
  "link.pick_up": "Dossier parent",
  "link.pick_home": "Accueil",
  "link.pick_choose": "Choisir ce dossier",
  "link.pick_new": "Nouveau dossier",
  "link.pick_new_name": "Nom du dossier",
  "link.pick_create": "Créer",
  "link.pick_cancel": "Annuler",
  "link.pick_empty": "Aucun sous-dossier",
  "link.pick_hidden": "Dossiers cachés",
  "link.pick_loading": "Lecture du dossier…",
} satisfies Record<keyof typeof source, string>;
