/**
 * Portuguese strings: connecting to a server.
 */

import type { link as source } from "../ru/link";

export const link = {
  "link.title": "Conectar a um servidor Yaca",
  "link.intro": "O Yaca roda no seu servidor e este aplicativo é uma janela para ele. Informe o endereço e o token que o script de instalação forneceu.",
  "link.address": "Endereço do servidor",
  "link.address_hint": "Por exemplo, yaca.example.com",
  "link.token": "Token",
  "link.token_hint": "Impresso por yaca-server token add.",
  "link.connect": "Conectar",
  "link.connecting": "Conectando…",
  "link.local": "Trabalhar neste computador",
  "link.err_unreachable": "O servidor não responde. Verifique o endereço e a rede.",
  "link.err_unauthorized": "O servidor não aceitou o token.",
  "link.err_bad_address": "Isso não parece um endereço de servidor.",
  "link.err_replaced": "Outro dispositivo se conectou ao servidor e levou os chats. Conecte-se de novo para trazê-los de volta.",
  "link.err_revoked": "O token deste dispositivo foi revogado.",
  "link.lost": "Conexão com o servidor perdida. Reconectando…",
  "link.settings_nav": "Conexão",
  "link.settings_title": "Conexão",
  "link.settings_this_pc": "O trabalho acontece neste computador.",
  "link.settings_server": "O trabalho acontece no servidor {name}.",
  "link.settings_to_server": "Conectar a um servidor",
  "link.settings_to_local": "Voltar a este computador",
  "link.settings_disconnect": "Desconectar do servidor",
  "link.settings_reload_note": "A janela será recarregada. Os chats no servidor continuam rodando.",
  "link.pick_up": "Subir",
  "link.pick_home": "Início",
  "link.pick_choose": "Escolher esta pasta",
  "link.pick_new": "Nova pasta",
  "link.pick_new_name": "Nome da pasta",
  "link.pick_create": "Criar",
  "link.pick_cancel": "Cancelar",
  "link.pick_empty": "Não há pastas aqui",
  "link.pick_hidden": "Pastas ocultas",
  "link.pick_broad": "Parece uma pasta pessoal ou a raiz, não um projeto. O agente trabalharia com tudo o que há nela e os instantâneos para reverter não vão funcionar.",
  "link.pick_loading": "Lendo a pasta…",
} satisfies Record<keyof typeof source, string>;
