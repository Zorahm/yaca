/**
 * Spanish strings: the model router — the settings section, the feed note and the
 * switcher's notice.
 */

import type { routing as source } from "../ru/routing";

export const routing = {
  "routing.title": "Enrutador",
  "routing.lede":
    "Un modelo pequeño responde preguntas sobre cada mensaje y unas reglas eligen qué modelo contesta. El archivo routing.json está junto a providers.json.",
  "routing.state_absent": "Sin configurar.",
  "routing.state_ready": "Conectado: en la lista de modelos aparece como «{name}».",
  "routing.state_broken": "Sin conectar: {error}",
  "routing.editor": "Contenido de routing.json",
  "routing.save": "Guardar",
  "routing.remove": "Quitar el enrutador",
  "routing.saved": "Guardado.",
  "routing.next_chat":
    "Los chats ya abiertos siguen como estaban; un chat nuevo toma el enrutador de este archivo.",
  "routing.default_route": "predeterminada",
  "routing.chip": "Ruta: {route} → {model}",
  "routing.menu_broken": "El enrutador no está conectado: {error}",
  "routing.wizard_title": "Configuración rápida",
  "routing.wizard_lede":
    "Elija tres modelos y el editor se rellena con una configuración lista: el modelo potente toma las peticiones difíciles y todo lo demás va al modelo predeterminado. No se escribe nada hasta que guarde.",
  "routing.wizard_decider": "Modelo que responde a las preguntas",
  "routing.wizard_default": "Modelo predeterminado",
  "routing.wizard_strong": "Modelo potente para lo difícil",
  "routing.wizard_build": "Rellenar el editor",
  "routing.wizard_none": "Aún no hay modelos: conecte primero un proveedor.",
  "routing.wizard_pick": "Elegir un modelo",
  "routing.decision_hint":
    "Lo mejor es tomar el modelo de decisión del proveedor «Decision» de arriba: indique su dirección, local o en la nube, y consulte sus modelos.",
} satisfies Record<keyof typeof source, string>;
