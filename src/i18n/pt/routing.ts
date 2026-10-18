/**
 * Portuguese strings: the model router — the settings section, the feed note and the
 * switcher's notice.
 */

import type { routing as source } from "../ru/routing";

export const routing = {
  "routing.title": "Roteador",
  "routing.lede":
    "Um modelo pequeno responde a perguntas sobre cada mensagem e regras escolhem qual modelo responde. O arquivo routing.json fica ao lado de providers.json.",
  "routing.state_absent": "Não configurado.",
  "routing.state_ready": "Conectado: na lista de modelos ele aparece como “{name}”.",
  "routing.state_broken": "Não conectado: {error}",
  "routing.editor": "Conteúdo de routing.json",
  "routing.save": "Salvar",
  "routing.remove": "Remover o roteador",
  "routing.saved": "Salvo.",
  "routing.next_chat":
    "Os chats já abertos continuam como estavam; um novo chat usa o roteador deste arquivo.",
  "routing.default_route": "padrão",
  "routing.chip": "Rota: {route} → {model}",
  "routing.menu_broken": "O roteador não está conectado: {error}",
  "routing.wizard_title": "Configuração rápida",
  "routing.wizard_lede":
    "Escolha três modelos e o editor é preenchido com uma configuração pronta: o modelo forte assume os pedidos difíceis e o resto vai para o modelo padrão. Nada é gravado até você salvar.",
  "routing.wizard_decider": "Modelo que responde às perguntas",
  "routing.wizard_default": "Modelo padrão",
  "routing.wizard_strong": "Modelo forte para o difícil",
  "routing.wizard_build": "Preencher o editor",
  "routing.wizard_none": "Ainda não há modelos: conecte um provedor primeiro.",
  "routing.wizard_pick": "Escolher um modelo",
  "routing.decision_hint":
    "O modelo de decisão é melhor obtido do provedor “Decision” acima: informe o endereço, local ou na nuvem, e consulte os modelos.",
} satisfies Record<keyof typeof source, string>;
