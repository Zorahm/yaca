/**
 * Russian strings: the frame of the window — sidebar, top bar, title bar,
 * menus, home screen. The source of these keys: `../en/shell.ts` has to cover
 * them.
 */

export const shell = {
  // --- sidebar ---------------------------------------------------------
  "side.new": "Новый чат",
  "side.projects": "Проекты",
  "side.new_here": "Новый чат в проекте",
  "side.toggle": "Боковая панель",
  "side.add_project": "Добавить проект",
  "side.forget": "Убрать проект",
  "side.forget_busy": "Сначала закройте чаты этого проекта",
  "side.no_projects": "Проектов пока нет",
  "side.working": "Работает",
  "side.waiting": "Ждёт ответа",
  "side.close_chat": "Закрыть чат",

  // --- top bar ---------------------------------------------------------
  "top.menu": "Меню приложения",
  "top.sessions": "Сессии",
  "top.search": "Поиск по сессии",
  "top.back": "Назад",
  "top.forward": "Вперёд",
  "top.processes": "Процессы",
  "top.checkpoints": "Чекпоинты",
  "panel.tab_mcp": "MCP",
  "panel.tab_lsp": "LSP",
  "panel.toggle": "Правая панель",
  "panel.resize": "Ширина панели",
  "panel.tab_agents": "Агенты",
  "side.resize": "Ширина боковой панели",
  "top.session_menu": "Меню сессии",
  "top.untitled": "Новая сессия",
  "top.rename_hint": "Двойной клик — переименовать",

  // --- home screen -----------------------------------------------------
  "home.greeting": "С чего начнём?",
  "home.greeting_named": "С чего начнём, {name}?",
  "home.pick_project": "Выберите папку",
  "home.pick_title": "Папка проекта",
  "home.not_chosen": "не выбрана",
  "home.lead":
    "Выберите папку проекта, и агент начнёт работу с её файлами. Режим, модель и усилие можно поменять до первого сообщения.",
  "home.no_provider": "Нет провайдера.",
  "home.no_provider_hint": "Добавьте ключ модели, чтобы писать сообщения.",

  "title.minimize": "Свернуть",
  "title.maximize": "Развернуть",
  "title.restore": "Восстановить",
  "title.close": "Закрыть",
  "menu.help": "Справка",
  "menu.check_updates": "Проверить наличие обновлений",
  "menu.about": "О программе",

  // --- common ----------------------------------------------------------
  "common.open_project": "Открыть проект",
  "common.close": "Закрыть",
  "common.seconds": "{n} с",
} as const;
