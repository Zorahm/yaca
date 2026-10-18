/**
 * Spanish strings: providers — the settings section and the panel before it.
 */

import type { providers as source } from "../ru/providers";

export const providers = {
  // Providers: the panel that exists until the settings window.
  "prov.title": "Proveedores",
  "prov.home_button": "Configurar proveedores",
  "prov.add": "Añadir proveedor",
  "prov.edit_row": "Configurar",
  "prov.delete": "Eliminar",
  "prov.delete_confirm": "¿Eliminar para siempre?",
  "prov.models_count.one": "{n} modelo",
  "prov.models_count.few": "{n} modelos",
  "prov.models_count.many": "{n} modelos",

  "prov.form_new": "Nuevo proveedor",
  "prov.form_name": "Nombre",
  "prov.form_more": "Avanzado",
  "prov.form_protocol": "Protocolo",
  "prov.form_base_url": "URL base",
  "prov.form_base_url_hint":
    "La ruta del protocolo se añade sola: /v1/chat/completions en OpenAI, /v1/messages en Anthropic. Para OpenRouter es https://openrouter.ai/api. Si la dirección ya incluye una versión (/v1, /v1beta, /v4), no se añade /v1.",
  "prov.form_headers": "Cabeceras propias",
  "prov.form_headers_hint":
    "Se añaden a las cabeceras del protocolo y pueden reemplazarlas, incluida authorization. Un {key} en el valor recibe la clave del llavero; la clave en sí nunca llega al archivo. Un valor vacío elimina la cabecera del protocolo.",
  "prov.form_header_name": "Nombre",
  "prov.form_header_value": "Valor",
  "prov.form_add_header": "Añadir cabecera",
  "prov.form_remove_header": "Quitar cabecera",

  "prov.form_key_title": "Clave",
  "prov.form_key_hint":
    "Se guarda en el llavero del sistema con el nombre del proveedor; providers.json nunca la contiene",
  "prov.form_key_new_hint":
    "Tras guardarlo, al proveedor se le pedirá su lista de modelos: lo que responda es lo que aparecerá en el selector de modelo",
  "prov.key_save": "Guardar clave",
  "prov.key_forget": "Olvidar clave",
  "prov.key_source_keychain": "clave en el llavero del sistema",
  "prov.key_source_environment": "clave en una variable de entorno",
  "prov.key_source_none": "sin clave",
  "prov.key_source_unknown": "el origen se verá cuando el proveedor esté guardado",
  "prov.key_not_needed": "No hace falta clave: {key} no aparece en ninguna cabecera",

  "prov.form_models": "Modelos",
  "prov.form_models_hint":
    "La lista la da el propio proveedor: se le consulta en cuanto se guarda la clave, y «Consultar» vuelve a preguntar. Con lo que responda se trabaja; el precio va en dólares por millón de tokens, y cero significa «gratis o desconocido».",
  "prov.form_model_context": "Contexto, tokens",
  "prov.form_model_output": "Salida máx., tokens",
  "prov.form_model_effort": "admite esfuerzo",
  "prov.models_ask": "Consultar",
  "prov.models_asking": "Consultando…",
  "prov.models_none": "No hay modelos",
  "prov.models_price": "$ / 1M entrada y salida",
  "prov.models_effort": "esfuerzo",

  "prov.form_save": "Guardar proveedor",
  "prov.form_cancel": "Cancelar",

  "prov.saved_unasked":
    "«{name}» está guardado, pero no respondió con su lista de modelos: {error}",
  "prov.error_name": "Indica el nombre del proveedor",
  "prov.error_url": "Indica la URL base del proveedor",
  "prov.error_header_name": "La cabecera {n} no tiene nombre",
  "prov.error_header_dup": "La cabecera {name} aparece dos veces",

  "prov.lede": "Adónde van la conversación y la clave. Es de nivel de aplicación: un proyecto no lo decide.",
  "prov.answered": "Respondió con su lista de modelos",
  "prov.unasked": "todavía no consultado",
  "prov.unasked_note":
    "Un círculo vacío significa que el proveedor nunca respondió con una lista de modelos. No aparecerá en el selector de modelo: la sesión no tendría con qué enviar la petición.",
} satisfies Record<keyof typeof source, string>;
