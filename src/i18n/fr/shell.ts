/**
 * French strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen.
 */

import type { shell as source } from "../ru/shell";

export const shell = {
  "side.new": "Nouveau chat",
  "side.projects": "Projets",
  "side.new_here": "Nouveau chat dans ce projet",
  "side.toggle": "Barre latérale",
  "side.add_project": "Ajouter un projet",
  "side.forget": "Retirer le projet",
  "side.forget_busy": "Fermez d'abord les chats de ce projet",
  "side.no_projects": "Aucun projet pour l'instant",
  "side.working": "En cours",
  "side.waiting": "En attente d'une réponse",
  "side.close_chat": "Fermer le chat",

  "top.menu": "Menu de l'application",
  "top.sessions": "Sessions",
  "top.search": "Rechercher dans la session",
  "top.back": "Retour",
  "top.forward": "Avancer",
  "top.processes": "Processus",
  "top.checkpoints": "Points de contrôle",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "Panneau de droite",
  "panel.resize": "Largeur du panneau",
  "panel.tab_agents": "Agents",
  "side.resize": "Largeur de la barre latérale",
  "top.session_menu": "Menu de la session",
  "top.untitled": "Nouvelle session",
  "top.rename_hint": "Double-clic pour renommer",

  "home.greeting": "Par où commencer ?",
  "home.greeting_named": "Par où commencer, {name} ?",
  "home.pick_project": "Choisissez un dossier",
  "home.pick_title": "Dossier du projet",
  "home.not_chosen": "non choisi",
  "home.lead":
    "Choisissez le dossier d'un projet, et l'agent se met à travailler avec ses fichiers. Le mode, le modèle et l'effort peuvent être changés avant le premier message.",
  "home.no_provider": "Aucun fournisseur.",
  "home.no_provider_hint": "Ajoutez une clé de modèle pour pouvoir écrire des messages.",

  "title.minimize": "Réduire",
  "title.maximize": "Agrandir",
  "title.restore": "Restaurer",
  "title.close": "Fermer",
  "menu.help": "Aide",
  "menu.check_updates": "Rechercher les mises à jour",
  "menu.about": "À propos de Yaca",

  "common.open_project": "Ouvrir un projet",
  "common.close": "Fermer",
  "common.seconds": "{n} s",
} satisfies Record<keyof typeof source, string>;
