/**
 * Russian strings: MCP servers — the settings section and the panel. The
 * source of these keys: `../en/mcp.ts` has to cover them.
 */

export const mcp = {
  "mcp.title": "Серверы MCP",
  "mcp.lede_app":
    "Ваши серверы: подключаются в каждом проекте. Их инструменты агент получает наравне со встроенными.",
  "mcp.lede_project":
    "Серверы этого проекта. Лежат в .yaca/settings.json и перекрывают одноимённые из общего списка и из .mcp.json.",
  "mcp.add": "Добавить сервер",
  "mcp.none_declared": "Серверов нет.",
  "mcp.edit": "Изменить",
  "mcp.enabled": "Подключать {name}",
  "mcp.delete": "Удалить",
  "mcp.delete_confirm": "Удалить?",
  "mcp.file_title": "Из .mcp.json",
  "mcp.file_hint":
    "Файл проекта, который читают и другие инструменты. Здесь он только показан. Его серверы запускаются после одобрения в чате: /mcp.",
  "mcp.file_replaced": "перекрыт сервером с тем же именем выше",
  "mcp.next_chat_note":
    "Список читается, когда чат открывается. Изменения доходят до следующего чата, а открытый работает с теми серверами, с которыми начал.",
  "mcp.form_new": "Новый сервер",
  "mcp.form_save": "Сохранить сервер",
  "mcp.form_name": "Имя",
  "mcp.form_name_hint":
    "Входит в имя каждого инструмента сервера. Латиница, цифры, дефис и подчёркивание.",
  "mcp.form_kind": "Как подключаться",
  "mcp.kind_stdio": "Программа",
  "mcp.kind_http": "Адрес",
  "mcp.form_command": "Программа",
  "mcp.form_command_hint": "Запускается напрямую, без оболочки.",
  "mcp.form_args": "Аргументы",
  "mcp.form_args_hint": "По одному на строку.",
  "mcp.form_env": "Окружение",
  "mcp.form_env_hint":
    "ИМЯ=значение, по одной на строку. Значение может ссылаться на переменную окружения, как в примере: так токен не попадает в файл.",
  "mcp.form_cwd": "Рабочий каталог",
  "mcp.form_cwd_hint": "Пусто — корень проекта.",
  "mcp.form_url": "Адрес",
  "mcp.form_url_hint":
    "Сервер со Streamable HTTP. Вход через OAuth не поддерживается: токен задаётся заголовком.",
  "mcp.form_headers": "Заголовки",
  "mcp.form_headers_hint":
    "Имя: значение, по одному на строку. Значение может ссылаться на переменную окружения, как в примере.",
  "mcp.problem_name_empty": "Задайте имя.",
  "mcp.problem_name_chars": "В имени допустимы латиница, цифры, дефис и подчёркивание.",
  "mcp.problem_name_taken": "Сервер с таким именем уже есть.",
  "mcp.problem_command_empty": "Задайте программу.",
  "mcp.problem_env": "Каждая строка окружения — ИМЯ=значение.",
  "mcp.problem_url": "Адрес должен начинаться с http:// или https://.",
  "mcp.problem_headers": "Каждая строка заголовков — Имя: значение.",

  "mcp.panel_empty":
    "У этого чата нет серверов MCP. Объявите их в настройках — они появятся в следующем чате.",
  "mcp.status_connecting": "подключается",
  "mcp.status_ready": "готов",
  "mcp.status_failed": "сбой",
  "mcp.status_disabled": "выключен",
  "mcp.status_needs_approval": "ждёт одобрения",
  "mcp.source_global": "общий список",
  "mcp.source_project_file": ".mcp.json проекта",
  "mcp.source_project": "настройки проекта",
  "mcp.approve": "Одобрить",
  "mcp.reconnect": "Переподключить",
  "mcp.approval_note":
    "Сервер объявлен в .mcp.json проекта и ещё не запускался. Одобрение запустит программу выше и запомнится, пока объявление не изменится.",
  "mcp.tools.one": "{n} инструмент",
  "mcp.tools.few": "{n} инструмента",
  "mcp.tools.many": "{n} инструментов",
  "mcp.resources": "ресурсы",
  "mcp.tool_reads": "читает",
  "mcp.tool_writes": "пишет",
} as const;
