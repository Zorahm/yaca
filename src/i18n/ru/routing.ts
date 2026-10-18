/**
 * Russian strings: the model router — the settings section, the feed note and the
 * switcher's notice. The source of these keys: `../en/routing.ts` has to cover them.
 */

export const routing = {
  "routing.title": "Маршрутизатор",
  "routing.lede":
    "Маленькая модель отвечает на вопросы о каждом сообщении, а правила выбирают, какая модель ответит. Файл routing.json лежит рядом с providers.json.",
  "routing.state_absent": "Не настроен.",
  "routing.state_ready": "Подключён: в списке моделей он называется «{name}».",
  "routing.state_broken": "Не подключён: {error}",
  "routing.editor": "Содержимое routing.json",
  "routing.save": "Сохранить",
  "routing.remove": "Убрать маршрутизатор",
  "routing.saved": "Сохранено.",
  "routing.next_chat":
    "Уже открытые чаты работают как раньше; новый чат берёт маршрутизатор из этого файла.",
  "routing.default_route": "по умолчанию",
  "routing.chip": "Маршрут: {route} → {model}",
  "routing.menu_broken": "Маршрутизатор не подключён: {error}",
  "routing.wizard_title": "Быстрая настройка",
  "routing.wizard_lede":
    "Выберите три модели, и редактор заполнится готовой настройкой: сильная модель берёт сложные запросы, остальное идёт на модель по умолчанию. Ничего не записывается, пока вы не сохраните.",
  "routing.wizard_decider": "Модель, которая отвечает на вопросы",
  "routing.wizard_default": "Модель по умолчанию",
  "routing.wizard_strong": "Сильная модель для сложного",
  "routing.wizard_build": "Заполнить редактор",
  "routing.wizard_none": "Нет моделей: сначала подключите провайдера.",
  "routing.wizard_pick": "Выберите модель",
  "routing.decision_hint":
    "Модель-решатель лучше взять у провайдера «Decision» выше: укажите его адрес, локальный или облачный, и опросите модели.",
} as const;
