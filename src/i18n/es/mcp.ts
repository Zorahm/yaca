/**
 * Spanish strings: MCP servers — the settings section and the panel.
 */

import type { mcp as source } from "../ru/mcp";

export const mcp = {
  "mcp.title": "Servidores MCP",
  "mcp.lede_app":
    "Servidores propios: se conectan en cada proyecto. Sus herramientas llegan al agente junto a las integradas.",
  "mcp.lede_project":
    "Servidores de este proyecto. Están en .yaca/settings.json y reemplazan a los del mismo nombre de la lista general y de .mcp.json.",
  "mcp.add": "Añadir servidor",
  "mcp.none_declared": "No hay servidores.",
  "mcp.edit": "Editar",
  "mcp.enabled": "Conectar {name}",
  "mcp.delete": "Eliminar",
  "mcp.delete_confirm": "¿Eliminar?",
  "mcp.file_title": "Desde .mcp.json",
  "mcp.file_hint":
    "Archivo del proyecto que también leen otras herramientas. Aquí solo se muestra. Sus servidores se inician al aprobarlos en el chat: /mcp.",
  "mcp.file_replaced": "reemplazado por el servidor del mismo nombre de arriba",
  "mcp.next_chat_note":
    "La lista se lee al abrir un chat. Los cambios llegan al siguiente chat; un chat abierto sigue con los servidores con los que empezó.",
  "mcp.form_new": "Nuevo servidor",
  "mcp.form_save": "Guardar el servidor",
  "mcp.form_name": "Nombre",
  "mcp.form_name_hint":
    "Forma parte del nombre de cada herramienta del servidor. Letras latinas, dígitos, guion y guion bajo.",
  "mcp.form_kind": "Cómo conectar",
  "mcp.kind_stdio": "Programa",
  "mcp.kind_http": "Dirección",
  "mcp.form_command": "Programa",
  "mcp.form_command_hint": "Se inicia directamente, sin shell.",
  "mcp.form_args": "Argumentos",
  "mcp.form_args_hint": "Uno por línea.",
  "mcp.form_env": "Entorno",
  "mcp.form_env_hint":
    "NOMBRE=valor, una por línea. El valor puede apuntar a una variable de entorno, como en el ejemplo: así el token no llega al archivo.",
  "mcp.form_cwd": "Directorio de trabajo",
  "mcp.form_cwd_hint": "Vacío significa la raíz del proyecto.",
  "mcp.form_url": "Dirección",
  "mcp.form_url_hint":
    "Servidor con Streamable HTTP. El inicio de sesión mediante OAuth no se admite: el token se indica en una cabecera.",
  "mcp.form_headers": "Cabeceras",
  "mcp.form_headers_hint":
    "Nombre: valor, uno por línea. El valor puede apuntar a una variable de entorno, como en el ejemplo.",
  "mcp.problem_name_empty": "Indica un nombre.",
  "mcp.problem_name_chars": "El nombre admite letras latinas, dígitos, guion y guion bajo.",
  "mcp.problem_name_taken": "Ya existe un servidor con ese nombre.",
  "mcp.problem_command_empty": "Indica el programa.",
  "mcp.problem_env": "Cada línea del entorno tiene la forma NOMBRE=valor.",
  "mcp.problem_url": "La dirección debe empezar por http:// o https://.",
  "mcp.problem_headers": "Cada línea de cabeceras tiene la forma Nombre: valor.",

  "mcp.panel_empty":
    "Este chat no tiene servidores MCP. Decláralos en los ajustes y aparecerán en el siguiente chat.",
  "mcp.status_connecting": "conectando",
  "mcp.status_ready": "listo",
  "mcp.status_failed": "error",
  "mcp.status_disabled": "desactivado",
  "mcp.status_needs_approval": "esperando aprobación",
  "mcp.source_global": "lista general",
  "mcp.source_project_file": ".mcp.json del proyecto",
  "mcp.source_project": "ajustes del proyecto",
  "mcp.source_plugin": "plugin",
  "mcp.approve": "Aprobar",
  "mcp.reconnect": "Volver a conectar",
  "mcp.status_needs_login": "requiere inicio de sesión",
  "mcp.status_logging_in": "iniciando sesión",
  "mcp.login": "Iniciar sesión",
  "mcp.logout": "Cerrar sesión",
  "mcp.login_note": "Termina de iniciar sesión en el navegador; el servidor se conectará después.",
  "mcp.login_open": "Abrir la página de inicio de sesión",
  "lsp.panel_empty": "No hay servidores de lenguaje en este chat. Los integrados arrancan si su programa está en el PATH; otros se añaden en Ajustes.",
  "lsp.status_idle": "sin iniciar",
  "lsp.status_starting": "iniciando",
  "lsp.status_ready": "listo",
  "lsp.status_failed": "falló",
  "lsp.status_disabled": "desactivado",
  "lsp.restart": "Reiniciar",
  "lsp.start": "Iniciar",
  "lsp.extensions": "Archivos: {list}",
  "mcp.approval_note":
    "El servidor está declarado en el .mcp.json del proyecto y todavía no se ha iniciado. La aprobación inicia el programa de arriba y se recuerda hasta que cambie la declaración.",
  "mcp.tools.one": "{n} herramienta",
  "mcp.tools.few": "{n} herramientas",
  "mcp.tools.many": "{n} herramientas",
  "mcp.resources": "recursos",
  "mcp.tool_reads": "lee",
  "mcp.tool_writes": "escribe",
} satisfies Record<keyof typeof source, string>;
