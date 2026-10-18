/**
 * French strings: MCP servers — the settings section and the panel.
 */

import type { mcp as source } from "../ru/mcp";

export const mcp = {
  "mcp.title": "Serveurs MCP",
  "mcp.lede_app":
    "Vos serveurs : ils se connectent dans chaque projet. L'agent reçoit leurs outils au même titre que les outils intégrés.",
  "mcp.lede_project":
    "Serveurs de ce projet. Ils sont stockés dans .yaca/settings.json et remplacent les serveurs de même nom de votre liste et de .mcp.json.",
  "mcp.add": "Ajouter un serveur",
  "mcp.none_declared": "Aucun serveur.",
  "mcp.edit": "Modifier",
  "mcp.enabled": "Connecter {name}",
  "mcp.delete": "Supprimer",
  "mcp.delete_confirm": "Supprimer ?",
  "mcp.file_title": "Depuis .mcp.json",
  "mcp.file_hint":
    "Fichier du projet, que lisent aussi d'autres outils. Il est seulement affiché ici. Ses serveurs démarrent après approbation dans le chat : /mcp.",
  "mcp.file_replaced": "remplacé par le serveur du même nom ci-dessus",
  "mcp.next_chat_note":
    "La liste est lue à l'ouverture d'un chat. Une modification s'applique au chat suivant ; un chat ouvert garde les serveurs avec lesquels il a démarré.",
  "mcp.form_new": "Nouveau serveur",
  "mcp.form_save": "Enregistrer le serveur",
  "mcp.form_name": "Nom",
  "mcp.form_name_hint":
    "Fait partie du nom de chaque outil du serveur. Lettres latines, chiffres, tiret et tiret bas.",
  "mcp.form_kind": "Mode de connexion",
  "mcp.kind_stdio": "Programme",
  "mcp.kind_http": "Adresse",
  "mcp.form_command": "Programme",
  "mcp.form_command_hint": "Lancé directement, sans shell.",
  "mcp.form_args": "Arguments",
  "mcp.form_args_hint": "Un par ligne.",
  "mcp.form_env": "Environnement",
  "mcp.form_env_hint":
    "NOM=valeur, une par ligne. Une valeur peut faire référence à une variable d'environnement, comme dans l'exemple : ainsi le jeton n'est pas écrit dans le fichier.",
  "mcp.form_cwd": "Dossier de travail",
  "mcp.form_cwd_hint": "Vide : la racine du projet.",
  "mcp.form_url": "Adresse",
  "mcp.form_url_hint":
    "Serveur utilisant Streamable HTTP. La connexion par OAuth n'est pas prise en charge : le jeton passe dans un en-tête.",
  "mcp.form_headers": "En-têtes",
  "mcp.form_headers_hint":
    "Nom : valeur, une par ligne. Une valeur peut faire référence à une variable d'environnement, comme dans l'exemple.",
  "mcp.problem_name_empty": "Donnez un nom.",
  "mcp.problem_name_chars": "Le nom peut contenir des lettres latines, des chiffres, un tiret et un tiret bas.",
  "mcp.problem_name_taken": "Un serveur porte déjà ce nom.",
  "mcp.problem_command_empty": "Indiquez le programme.",
  "mcp.problem_env": "Chaque ligne d'environnement doit être de la forme NOM=valeur.",
  "mcp.problem_url": "L'adresse doit commencer par http:// ou https://.",
  "mcp.problem_headers": "Chaque ligne d'en-tête doit être de la forme Nom : valeur.",

  "mcp.panel_empty":
    "Ce chat n'a pas de serveurs MCP. Déclarez-les dans les paramètres : ils apparaîtront dans le prochain chat.",
  "mcp.status_connecting": "connexion en cours",
  "mcp.status_ready": "prêt",
  "mcp.status_failed": "échec",
  "mcp.status_disabled": "désactivé",
  "mcp.status_needs_approval": "en attente d'approbation",
  "mcp.source_global": "votre liste",
  "mcp.source_project_file": ".mcp.json du projet",
  "mcp.source_project": "paramètres du projet",
  "mcp.source_plugin": "plugin",
  "mcp.approve": "Approuver",
  "mcp.reconnect": "Reconnecter",
  "mcp.status_needs_login": "connexion requise",
  "mcp.status_logging_in": "connexion en cours",
  "mcp.login": "Se connecter",
  "mcp.logout": "Se déconnecter",
  "mcp.login_note": "Terminez la connexion dans le navigateur ; le serveur se connectera ensuite.",
  "mcp.login_open": "Ouvrir la page de connexion",
  "lsp.panel_empty": "Aucun serveur de langage dans ce chat. Les serveurs intégrés démarrent si leur programme est dans le PATH ; les autres s’ajoutent dans les Réglages.",
  "lsp.status_idle": "non démarré",
  "lsp.status_starting": "démarrage",
  "lsp.status_ready": "prêt",
  "lsp.status_failed": "échec",
  "lsp.status_disabled": "désactivé",
  "lsp.restart": "Redémarrer",
  "lsp.start": "Démarrer",
  "lsp.extensions": "Fichiers : {list}",
  "mcp.approval_note":
    "Déclaré dans le .mcp.json du projet et pas encore démarré. L'approbation lance le programme indiqué ci-dessus et est mémorisée jusqu'à la modification de la déclaration.",
  "mcp.tools.one": "{n} outil",
  "mcp.tools.few": "{n} outils",
  "mcp.tools.many": "{n} outils",
  "mcp.resources": "ressources",
  "mcp.tool_reads": "lit",
  "mcp.tool_writes": "écrit",
} satisfies Record<keyof typeof source, string>;
