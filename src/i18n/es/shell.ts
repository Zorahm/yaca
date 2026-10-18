/**
 * Spanish strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen.
 */

import type { shell as source } from "../ru/shell";

export const shell = {
  "side.new": "Nuevo chat",
  "side.projects": "Proyectos",
  "side.new_here": "Nuevo chat en este proyecto",
  "side.toggle": "Barra lateral",
  "side.update_do": "Actualizar",
  "side.update_more": "Detalles",
  "side.forget": "Quitar proyecto",
  "side.forget_busy": "Primero cierra los chats de este proyecto",
  "side.no_projects": "Todavía no hay proyectos",
  "side.working": "Trabajando",
  "side.waiting": "Esperando respuesta",
  "side.close_chat": "Cerrar chat",

  "top.menu": "Menú de la aplicación",
  "top.sessions": "Sesiones",
  "top.search": "Buscar en la sesión",
  "top.back": "Atrás",
  "top.forward": "Adelante",
  "top.processes": "Procesos",
  "top.checkpoints": "Puntos de control",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "Panel derecho",
  "panel.resize": "Anchura del panel",
  "panel.tab_agents": "Agentes",
  "side.resize": "Anchura de la barra lateral",
  "top.session_menu": "Menú de la sesión",
  "top.untitled": "Nueva sesión",
  "top.rename_hint": "Doble clic para renombrar",

  "home.greeting": "¿Por dónde empezamos?",
  "home.greeting_named": "¿Por dónde empezamos, {name}?",
  "home.pick_project": "Elige una carpeta",
  "home.pick_title": "Carpeta del proyecto",
  "home.not_chosen": "sin elegir",
  "home.lead":
    "Elige la carpeta del proyecto y el agente empezará a trabajar con sus archivos. El modo, el modelo y el esfuerzo se pueden cambiar antes del primer mensaje.",
  "home.no_provider": "No hay proveedor.",
  "home.no_provider_hint": "Añade una clave de modelo para poder escribir mensajes.",

  "title.minimize": "Minimizar",
  "title.maximize": "Maximizar",
  "title.restore": "Restaurar",
  "title.close": "Cerrar",
  "menu.help": "Ayuda",
  "menu.check_updates": "Buscar actualizaciones",
  "menu.about": "Acerca de Yaca",

  "common.open_project": "Abrir un proyecto",
  "common.close": "Cerrar",
  "common.seconds": "{n} s",
} satisfies Record<keyof typeof source, string>;
