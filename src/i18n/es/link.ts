/**
 * Spanish strings: connecting to a server.
 */

import type { link as source } from "../ru/link";

export const link = {
  "link.title": "Conectar con un servidor de Yaca",
  "link.intro": "Yaca se ejecuta en tu servidor y esta aplicación es una ventana hacia él. Introduce la dirección y el token que dio el script de instalación.",
  "link.address": "Dirección del servidor",
  "link.address_hint": "Por ejemplo, yaca.example.com",
  "link.token": "Token",
  "link.token_hint": "Lo imprime yaca-server token add.",
  "link.connect": "Conectar",
  "link.connecting": "Conectando…",
  "link.local": "Trabajar en este equipo",
  "link.err_unreachable": "El servidor no responde. Revisa la dirección y la red.",
  "link.err_unauthorized": "El servidor no aceptó el token.",
  "link.err_bad_address": "Eso no parece una dirección de servidor.",
  "link.err_replaced": "Otro dispositivo se conectó al servidor y se llevó los chats. Conéctate de nuevo para traerlos aquí.",
  "link.err_revoked": "El token de este dispositivo ha sido revocado.",
  "link.lost": "Se perdió la conexión con el servidor. Reconectando…",
  "link.settings_nav": "Conexión",
  "link.settings_title": "Conexión",
  "link.settings_this_pc": "El trabajo se hace en este equipo.",
  "link.settings_server": "El trabajo se hace en el servidor {name}.",
  "link.settings_to_server": "Conectar con un servidor",
  "link.settings_to_local": "Volver a este equipo",
  "link.settings_disconnect": "Desconectar del servidor",
  "link.settings_reload_note": "La ventana se recargará. Los chats del servidor seguirán en marcha.",
  "link.pick_up": "Subir",
  "link.pick_home": "Inicio",
  "link.pick_choose": "Elegir esta carpeta",
  "link.pick_new": "Carpeta nueva",
  "link.pick_new_name": "Nombre de la carpeta",
  "link.pick_create": "Crear",
  "link.pick_cancel": "Cancelar",
  "link.pick_empty": "No hay carpetas aquí",
  "link.pick_hidden": "Carpetas ocultas",
  "link.pick_broad": "Parece una carpeta personal o la raíz, no un proyecto. El agente trabajaría con todo lo que contiene y las instantáneas para deshacer no funcionarán.",
  "link.pick_loading": "Leyendo la carpeta…",
} satisfies Record<keyof typeof source, string>;
