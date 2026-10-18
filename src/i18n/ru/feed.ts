/**
 * Russian strings: the feed — action rows, group summaries, notices, errors,
 * compaction. The source of these keys: `../en/feed.ts` has to cover them.
 */

export const feed = {
  // --- feed ------------------------------------------------------------
  "feed.you": "Вы",
  "feed.agent": "Yaca",
  "feed.jump": "К новым",
  "feed.copy": "Копировать",
  "feed.copied": "Скопировано",
  "shown.lines": "строки {from}–{to} из {total}",

  // --- thinking --------------------------------------------------------
  "work.thought": "Размышление",
  "work.thinking": "Размышляет…",

  // --- summary header of an action group -------------------------------
  // The first category carries the noun; after it the noun has been said:
  // «Создано 7 файлов, изменён 1, выполнено 9 команд».
  "group.created.with.one": "создан {n} файл",
  "group.created.with.few": "создано {n} файла",
  "group.created.with.many": "создано {n} файлов",
  "group.created.bare.one": "создан {n}",
  "group.created.bare.few": "создано {n}",
  "group.created.bare.many": "создано {n}",
  "group.modified.with.one": "изменён {n} файл",
  "group.modified.with.few": "изменено {n} файла",
  "group.modified.with.many": "изменено {n} файлов",
  "group.modified.bare.one": "изменён {n}",
  "group.modified.bare.few": "изменено {n}",
  "group.modified.bare.many": "изменено {n}",
  "group.deleted.with.one": "удалён {n} файл",
  "group.deleted.with.few": "удалено {n} файла",
  "group.deleted.with.many": "удалено {n} файлов",
  "group.deleted.bare.one": "удалён {n}",
  "group.deleted.bare.few": "удалено {n}",
  "group.deleted.bare.many": "удалено {n}",
  "group.commands.one": "выполнена {n} команда",
  "group.commands.few": "выполнено {n} команды",
  "group.commands.many": "выполнено {n} команд",
  "group.tools.one": "вызван {n} инструмент",
  "group.tools.few": "вызвано {n} инструмента",
  "group.tools.many": "вызвано {n} инструментов",
  "group.failed.one": "{n} с ошибкой",
  "group.failed.few": "{n} с ошибкой",
  "group.failed.many": "{n} с ошибкой",
  "group.expand": "Показать действия",
  "group.collapse": "Свернуть действия",

  // --- captions the interface writes -----------------------------------
  "act.created": "Создан {name}",
  "act.modified": "Изменён {name}",
  "act.deleted": "Удалён {name}",
  "act.deleted_dir": "Удалён каталог {name}",
  "act.moved": "Перемещён {from} → {to}",
  "act.read": "Прочитан {name}",
  "act.reading": "Читаю {name}",
  "act.creating": "Создаю {name}",
  "act.modifying": "Изменяю {name}",
  "act.deleting": "Удаляю {name}",
  "act.deleting_dir": "Удаляю каталог {name}",
  "act.moving": "Перемещаю {from} → {to}",
  "act.edit_failed": "Не удалось изменить {name}",
  "act.failed": "Не удалось выполнить действие",
  "act.rejected": "Правка отклонена · {reason}",
  "act.denied": "Отклонено вами",
  "act.stopped": "Остановлено вами",
  "act.denied_why": "Отклонено вами · {reason}",
  "act.cancelled": "Отменено",
  "act.applied": "Применено к рабочему дереву",
  "act.restored": "Откачено к {time}",
  "act.running": "Выполняю",
  "act.done": "Выполнено",

  // --- expanded row ----------------------------------------------------
  "row.diff_truncated": "Дифф обрезан ядром",
  "row.no_output": "Вывод пуст",
  "row.files.one": "{n} файл",
  "row.files.few": "{n} файла",
  "row.files.many": "{n} файлов",

  // --- errors ----------------------------------------------------------
  "err.retry": "Повторить",
  "err.change_model": "Сменить модель",
  "err.retry_in": "Повтор через {seconds} с · попытка {attempt} из {of}",
  "err.stopped": "Остановлено",
  "hook.blocked_prompt": "Хук плагина {plugin} не пропустил сообщение:",
  "hook.blocked_stop": "Хук плагина {plugin} вернул агента к работе:",
  "hook.said": "Хук плагина {plugin} ({event}):",
  "elicit.from": "MCP-сервер {server} просит:",
  "elicit.send": "Отправить",
  "elicit.decline": "Отклонить",
  "elicit.choose": "Выберите…",
  "elicit.accepted": "Отправлено серверу {server}",
  "elicit.declined": "Отклонено",
  "elicit.cancelled": "Закрыто без ответа",
  "elicit.expired": "Сервер перестал ждать",
  "hook.project_blocked_prompt": "Хук проекта не пропустил сообщение:",
  "hook.project_blocked_stop": "Хук проекта вернул агента к работе:",
  "hook.project_said": "Хук проекта ({event}):",
  "err.send_failed": "Не удалось отправить команду ядру",

  // --- history compaction ----------------------------------------------
  "compact.divider": "история сжата · освобождено ~{tokens} токенов",
  "compact.summary_title": "Сводка",
  "compact.edit": "Править сводку",
  "compact.save": "Сохранить сводку",
  "compact.cancel": "Отмена",
  "compact.edited": "сводка переписана",

  // --- notice bars -----------------------------------------------------
  "notice.context_full": "Контекст почти заполнен · сожмётся автоматически",
  "notice.compact": "Сжать",
  "notice.dismiss": "Убрать",

  // Durations for a process's running time. The core sends milliseconds;
  // the interface counts the hours.
  "dur.h": "{n} ч",
  "dur.m": "{n} мин",
  "dur.s": "{n} с",
  "turn.working": "Работает {time}",
  "turn.worked": "Работал {time}",

  "notice.mcp_approval": "Сервер MCP ждёт одобрения: {names}",
  "notice.mcp_failed": "Сервер MCP не подключился: {names}",
  "notice.mcp_login": "MCP-сервер просит войти: {names}",
  "notice.lsp_failed": "Языковой сервер не отвечает: {names}",
  "notice.open": "Открыть",

  // --- task list -------------------------------------------------------
  "todo.title": "Задачи",
} as const;
