/**
 * Brazilian Portuguese strings: providers — the settings section and the panel before it.
 */

import type { providers as source } from "../ru/providers";

export const providers = {
  // Providers: the panel that exists until the settings window.
  "prov.title": "Provedores",
  "prov.home_button": "Configurar provedores",
  "prov.add": "Adicionar provedor",
  "prov.edit_row": "Configurar",
  "prov.delete": "Excluir",
  "prov.delete_confirm": "Excluir definitivamente?",
  "prov.models_count.one": "{n} modelo",
  "prov.models_count.few": "{n} modelos",
  "prov.models_count.many": "{n} modelos",

  "prov.form_new": "Novo provedor",
  "prov.form_name": "Nome",
  "prov.form_more": "Avançado",
  "prov.form_protocol": "Protocolo",
  "prov.form_base_url": "URL base",
  "prov.form_base_url_hint":
    "O caminho do protocolo é acrescentado automaticamente: /v1/chat/completions na OpenAI, /v1/messages na Anthropic. Para o OpenRouter, é https://openrouter.ai/api. Se o endereço já traz uma versão (/v1, /v1beta, /v4), /v1 não é acrescentado.",
  "prov.form_headers": "Cabeçalhos próprios",
  "prov.form_headers_hint":
    "São somados aos cabeçalhos do protocolo e podem substituí-los — inclusive authorization. Um {key} no valor recebe a chave do chaveiro do sistema; a própria chave nunca vai para o arquivo. Um valor vazio remove o cabeçalho do protocolo.",
  "prov.form_header_name": "Nome",
  "prov.form_header_value": "Valor",
  "prov.form_add_header": "Adicionar cabeçalho",
  "prov.form_remove_header": "Remover cabeçalho",

  "prov.form_key_title": "Chave",
  "prov.form_key_hint":
    "Vai para o chaveiro do sistema com o nome do provedor — o providers.json nunca a contém",
  "prov.form_key_new_hint":
    "Depois de salvo, o provedor é consultado sobre os próprios modelos: o que ele responder é o que aparece no seletor de modelo",
  "prov.key_save": "Salvar chave",
  "prov.key_forget": "Esquecer chave",
  "prov.key_source_keychain": "chave no chaveiro do sistema",
  "prov.key_source_environment": "chave em variável de ambiente",
  "prov.key_source_none": "sem chave",
  "prov.key_source_unknown": "a origem aparece quando o provedor estiver salvo",
  "prov.key_not_needed": "Chave não necessária: {key} não aparece em nenhum cabeçalho",

  "prov.form_models": "Modelos",
  "prov.form_models_hint":
    "A lista vem do próprio provedor: ele é consultado assim que a chave é salva, e “Consultar” pergunta de novo. Com o que ele responder é que Yaca conversa; o preço é em dólares por milhão de tokens, e zero significa “gratuito ou desconhecido”.",
  "prov.form_model_context": "Contexto, tokens",
  "prov.form_model_output": "Saída máx., tokens",
  "prov.form_model_effort": "suporta esforço",
  "prov.models_ask": "Consultar",
  "prov.models_asking": "Consultando…",
  "prov.models_none": "Nenhum modelo",
  "prov.models_price": "$ / 1 M entrada e saída",
  "prov.models_effort": "esforço",

  "prov.form_save": "Salvar provedor",
  "prov.form_cancel": "Cancelar",

  "prov.saved_unasked":
    "“{name}” foi salvo, mas não respondeu com a lista de modelos: {error}",
  "prov.error_name": "Informe o nome do provedor",
  "prov.error_url": "Informe a URL base do provedor",
  "prov.error_header_name": "O cabeçalho {n} não tem nome",
  "prov.error_header_dup": "O cabeçalho {name} está listado duas vezes",

  "prov.lede": "Para onde vão a conversa e a chave. Nível do aplicativo: o projeto não decide isso.",
  "prov.answered": "Respondeu com a lista de modelos",
  "prov.unasked": "ainda não consultado",
  "prov.unasked_note":
    "Um ponto vazio significa que o provedor nunca respondeu com uma lista de modelos. Ele não aparece no seletor de modelo: a sessão não teria como enviar requisições.",
} satisfies Record<keyof typeof source, string>;
