/**
 * Russian strings: providers — the settings section and the panel before it.
 * The source of these keys: `../en/providers.ts` has to cover them.
 */

export const providers = {
  // --- providers: the panel that exists until the settings window ------
  "prov.title": "Провайдеры",
  "prov.home_button": "Настроить провайдера",
  "prov.add": "Добавить провайдера",
  "prov.edit_row": "Настроить",
  "prov.delete": "Удалить",
  "prov.delete_confirm": "Удалить навсегда?",
  "prov.models_count.one": "{n} модель",
  "prov.models_count.few": "{n} модели",
  "prov.models_count.many": "{n} моделей",

  "prov.form_new": "Новый провайдер",
  "prov.form_name": "Название",
  "prov.form_more": "Дополнительно",
  "prov.form_protocol": "Протокол",
  "prov.form_base_url": "Базовый URL",
  "prov.form_base_url_hint":
    "Путь протокола добавится сам: /v1/chat/completions у OpenAI, /v1/messages у Anthropic. Для OpenRouter это https://openrouter.ai/api. Если версия уже есть в адресе (/v1, /v1beta, /v4), /v1 не добавляется.",
  "prov.form_headers": "Свои заголовки",
  "prov.form_headers_hint":
    "Добавляются к заголовкам протокола и перебивают их — включая authorization. {key} в значении получит ключ из кейчейна; сам ключ в файл не пишется никогда. Пустое значение убирает заголовок протокола.",
  "prov.form_header_name": "Имя",
  "prov.form_header_value": "Значение",
  "prov.form_add_header": "Добавить заголовок",
  "prov.form_remove_header": "Убрать заголовок",

  "prov.form_key_title": "Ключ",
  "prov.form_key_hint":
    "Уезжает в системный кейчейн под именем провайдера — в providers.json его нет",
  "prov.form_key_new_hint":
    "После сохранения провайдер будет опрошен сам: что он ответит, то и появится в выборе модели",
  "prov.key_save": "Сохранить ключ",
  "prov.key_forget": "Забыть ключ",
  "prov.key_source_keychain": "ключ в системном кейчейне",
  "prov.key_source_environment": "ключ в переменной окружения",
  "prov.key_source_none": "ключа нет",
  "prov.key_source_unknown": "источник станет виден, когда провайдер сохранён",
  "prov.key_not_needed": "Ключ не нужен: {key} не встречается ни в одном заголовке",

  "prov.form_models": "Модели",
  "prov.form_models_hint":
    "Список приходит от самого провайдера: он опрашивается, как только сохранён ключ, а «Опросить» спрашивает ещё раз. Что он ответил, тем и разговариваем; цена — долларов за миллион токенов, ноль значит «бесплатно или неизвестно».",
  "prov.form_model_context": "Контекст, токенов",
  "prov.form_model_output": "Макс. вывод, токенов",
  "prov.form_model_effort": "поддерживает усилие",
  "prov.models_ask": "Опросить",
  "prov.models_asking": "Спрашиваем…",
  "prov.models_none": "Моделей нет",
  "prov.models_price": "$ / 1M вход и выход",
  "prov.models_effort": "усилие",

  "prov.form_save": "Сохранить провайдера",
  "prov.form_cancel": "Отмена",

  "prov.saved_unasked":
    "«{name}» сохранён, но списком моделей не ответил: {error}",
  "prov.error_name": "Укажите название",
  "prov.error_url": "Укажите базовый URL",
  "prov.error_header_name": "Заголовок {n} без имени",
  "prov.error_header_dup": "Заголовок {name} указан дважды",

  "prov.lede": "Куда уезжает разговор и ключ. Уровень приложения: проект этого не решает.",
  "prov.answered": "Ответил списком моделей",
  "prov.unasked": "не опрошен",
  "prov.unasked_note":
    "Пустой кружок — провайдер не ответил списком моделей. В выборе модели он не появится: сессии нечем отправить запрос.",
} as const;
