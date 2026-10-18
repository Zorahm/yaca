/**
 * Brazilian Portuguese strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen.
 */

import type { shell as source } from "../ru/shell";

export const shell = {
  "side.new": "Novo chat",
  "side.projects": "Projetos",
  "side.new_here": "Novo chat neste projeto",
  "side.toggle": "Barra lateral",
  "side.add_project": "Adicionar projeto",
  "side.forget": "Remover projeto",
  "side.forget_busy": "Feche antes os chats deste projeto",
  "side.no_projects": "Nenhum projeto ainda",
  "side.working": "Trabalhando",
  "side.waiting": "Aguardando resposta",
  "side.close_chat": "Fechar chat",

  "top.menu": "Menu do aplicativo",
  "top.sessions": "Sessões",
  "top.search": "Pesquisar nesta sessão",
  "top.back": "Voltar",
  "top.forward": "Avançar",
  "top.processes": "Processos",
  "top.checkpoints": "Checkpoints",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "Painel direito",
  "panel.resize": "Largura do painel",
  "panel.tab_agents": "Agentes",
  "side.resize": "Largura da barra lateral",
  "top.session_menu": "Menu da sessão",
  "top.untitled": "Nova sessão",
  "top.rename_hint": "Clique duplo para renomear",

  "home.greeting": "Por onde começamos?",
  "home.greeting_named": "Por onde começamos, {name}?",
  "home.pick_project": "Escolha uma pasta",
  "home.pick_title": "Pasta do projeto",
  "home.not_chosen": "não escolhida",
  "home.lead":
    "Escolha a pasta de um projeto e o agente começa a trabalhar com os arquivos dela. Modo, modelo e esforço podem ser alterados antes da primeira mensagem.",
  "home.no_provider": "Nenhum provedor.",
  "home.no_provider_hint": "Adicione uma chave de modelo para poder enviar mensagens.",

  "title.minimize": "Minimizar",
  "title.maximize": "Maximizar",
  "title.restore": "Restaurar",
  "title.close": "Fechar",
  "menu.help": "Ajuda",
  "menu.check_updates": "Verificar atualizações",
  "menu.about": "Sobre o Yaca",

  "common.open_project": "Abrir um projeto",
  "common.close": "Fechar",
  "common.seconds": "{n} s",
} satisfies Record<keyof typeof source, string>;
