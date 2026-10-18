/**
 * Brazilian Portuguese strings: MCP servers — the settings section and the panel.
 */

import type { mcp as source } from "../ru/mcp";

export const mcp = {
  "mcp.title": "Servidores MCP",
  "mcp.lede_app":
    "Seus servidores, conectados em todos os projetos. O agente recebe as ferramentas deles junto com as integradas.",
  "mcp.lede_project":
    "Os servidores deste projeto. Ficam no .yaca/settings.json e substituem os de mesmo nome da sua lista geral e do .mcp.json.",
  "mcp.add": "Adicionar servidor",
  "mcp.none_declared": "Nenhum servidor.",
  "mcp.edit": "Editar",
  "mcp.enabled": "Conectar {name}",
  "mcp.delete": "Excluir",
  "mcp.delete_confirm": "Excluir?",
  "mcp.file_title": "Do .mcp.json",
  "mcp.file_hint":
    "Arquivo do projeto, que outras ferramentas também leem. Aqui ele só é exibido. Os servidores dele iniciam depois de aprovados em um chat: /mcp.",
  "mcp.file_replaced": "substituído pelo servidor de mesmo nome acima",
  "mcp.next_chat_note":
    "A lista é lida quando um chat abre. Mudanças valem para o próximo chat; um chat aberto continua com os servidores com que começou.",
  "mcp.form_new": "Novo servidor",
  "mcp.form_save": "Salvar servidor",
  "mcp.form_name": "Nome",
  "mcp.form_name_hint":
    "Faz parte do nome de cada ferramenta do servidor. Letras latinas, dígitos, hífen e sublinhado.",
  "mcp.form_kind": "Como conectar",
  "mcp.kind_stdio": "Programa",
  "mcp.kind_http": "Endereço",
  "mcp.form_command": "Programa",
  "mcp.form_command_hint": "Iniciado diretamente, sem shell.",
  "mcp.form_args": "Argumentos",
  "mcp.form_args_hint": "Um por linha.",
  "mcp.form_env": "Ambiente",
  "mcp.form_env_hint":
    "NOME=valor, uma por linha. O valor pode apontar para uma variável de ambiente, como no exemplo: assim o token não vai para o arquivo.",
  "mcp.form_cwd": "Diretório de trabalho",
  "mcp.form_cwd_hint": "Vazio significa a raiz do projeto.",
  "mcp.form_url": "Endereço",
  "mcp.form_url_hint":
    "Servidor com Streamable HTTP. Login por OAuth não é suportado: o token é informado em um cabeçalho.",
  "mcp.form_headers": "Cabeçalhos",
  "mcp.form_headers_hint":
    "Nome: valor, um por linha. O valor pode apontar para uma variável de ambiente, como no exemplo.",
  "mcp.problem_name_empty": "Informe um nome.",
  "mcp.problem_name_chars": "O nome pode ter letras latinas, dígitos, hífen e sublinhado.",
  "mcp.problem_name_taken": "Já existe um servidor com este nome.",
  "mcp.problem_command_empty": "Informe o programa.",
  "mcp.problem_env": "Cada linha do ambiente é NOME=valor.",
  "mcp.problem_url": "O endereço deve começar com http:// ou https://.",
  "mcp.problem_headers": "Cada linha de cabeçalho é Nome: valor.",

  "mcp.panel_empty":
    "Este chat não tem servidores MCP. Declare-os nas configurações e eles aparecerão no próximo chat.",
  "mcp.status_connecting": "conectando",
  "mcp.status_ready": "pronto",
  "mcp.status_failed": "falhou",
  "mcp.status_disabled": "desligado",
  "mcp.status_needs_approval": "aguardando aprovação",
  "mcp.source_global": "sua lista geral",
  "mcp.source_project_file": ".mcp.json do projeto",
  "mcp.source_project": "configurações do projeto",
  "mcp.source_plugin": "plugin",
  "mcp.source_app": "aplicativo",
  "mcp.approve": "Aprovar",
  "mcp.reconnect": "Reconectar",
  "mcp.status_needs_login": "login necessário",
  "mcp.status_logging_in": "entrando",
  "mcp.login": "Entrar",
  "mcp.logout": "Sair",
  "mcp.login_note": "Conclua o login no navegador; o servidor se conecta em seguida.",
  "mcp.login_open": "Abrir a página de login",
  "lsp.panel_empty": "Não há servidores de linguagem neste chat. Os embutidos iniciam se o programa estiver no PATH; outros são adicionados nas Configurações.",
  "lsp.status_idle": "não iniciado",
  "lsp.status_starting": "iniciando",
  "lsp.status_ready": "pronto",
  "lsp.status_failed": "falhou",
  "lsp.status_disabled": "desligado",
  "lsp.restart": "Reiniciar",
  "lsp.start": "Iniciar",
  "lsp.extensions": "Arquivos: {list}",
  "mcp.approval_note":
    "Declarado no .mcp.json do projeto e ainda não iniciado. A aprovação inicia o programa acima e fica registrada até a declaração mudar.",
  "mcp.tools.one": "{n} ferramenta",
  "mcp.tools.few": "{n} ferramentas",
  "mcp.tools.many": "{n} ferramentas",
  "mcp.resources": "recursos",
  "mcp.tool_reads": "lê",
  "mcp.tool_writes": "escreve",
} satisfies Record<keyof typeof source, string>;
