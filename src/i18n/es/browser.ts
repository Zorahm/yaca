/**
 * Spanish strings: the browser column.
 */

import type { browser as source } from "../ru/browser";

export const browser = {
  "browser.toggle": "Navegador",
  "browser.back": "Atrás",
  "browser.forward": "Adelante",
  "browser.reload": "Recargar",
  "browser.stop": "Detener la carga",
  "browser.address": "Dirección o localhost:puerto",
  "browser.external": "Abrir en el navegador del sistema",
  "browser.devtools": "Herramientas de desarrollo",
  "browser.empty_title": "Todavía no hay nada abierto",
  "browser.empty_body": "Aquí aparecerá la página que abra usted o el modelo",
  "browser.size_model": "{size} · fijado por el modelo",
  "browser.reset_size": "Volver al tamaño de la columna",
  "browser.resize": "Cambiar el ancho del navegador",
  "browser.bad_address": "Eso no parece la dirección de una página",
  "browser.open_failed": "La página no se abrió",
  "browser.busy": "El modelo está usando el navegador",
} satisfies Record<keyof typeof source, string>;
