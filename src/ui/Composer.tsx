/**
 * Композер §7 и его нижний ряд §7.1.
 *
 * Поле не блокируется, пока агент работает (§7.4): отправленное встаёт в
 * очередь ядра и показывается чипом над полем. Чипы — список ядра целиком:
 * каждый показывает ожидающее сообщение, а `отменить` шлёт команду с его id.
 * Ядро отвечает новым списком, поэтому чип говорит только то, что правда.
 *
 * Настройки нижнего ряда приходят пропсом, а не берутся из `SessionState`:
 * §9.1 ставит тот же композер на домашний экран, где сессии ещё нет, а
 * выбранное там уезжает в неё вместе с первым сообщением.
 *
 * Чего здесь нет из §7.1: кнопки диктовки. Распознавания речи нет, а кнопка,
 * молча ничего не делающая, это интерфейс, врущий про состояние. `＋` есть:
 * диалог файлов у оболочки нашёлся. `@` (§7.2) есть: меню файлов проекта
 * (`./MentionMenu`) открывается, когда `@` начинает слово; выбранный файл
 * становится чипом над полем, как и приложенный через `＋`. На домашнем экране
 * поиска нет, и меню там не открывается вовсе.
 *
 * `/` в начале строки открывает меню команд §7.3 (`./slash`). Меню не
 * состояние, а функция от текста поля: `⇥` и `⏎` берут подсвеченное,
 * пробел закрывает — дальше идут аргументы.
 */

import { ArrowUp, Plus, Stop } from "@phosphor-icons/react";
import { useEffect, useRef, useState, type ClipboardEvent } from "react";

import type { Effort, Mode, ModelInfo, SlashCommand, UsageSnapshot } from "../core/client";
import type { Feed } from "../core/feed";
import type { QueuedMessage } from "../core/bindings/QueuedMessage";
import { t } from "../i18n";
import { EffortMenu } from "./EffortMenu";
import { ModeMenu, MODE_KEY } from "./ModeMenu";
import { ModelMenu } from "./ModelMenu";
import { MentionMenu, useMentions } from "./MentionMenu";
import { withoutMention, type Mention } from "./mention";
import { QueuedChip } from "./QueuedChip";
import { SlashMenu } from "./SlashMenu";
import { Attachment } from "./Attachment";
import { builtinFor, menuFor, type Builtin, type SlashItem } from "./slash";
import { StatusRing, type RingState } from "./StatusRing";


export type ComposerActions = {
  send: (text: string, attachments: string[]) => void;
  stop: () => void;
  setMode: (mode: Mode) => void;
  /**
   * Обход разрешений (§8) — решение о работающей сессии. Там, где сессии нет,
   * действия нет тоже, и меню режима не показывает строку, которой некуда
   * приложиться.
   */
  setBypass?: (on: boolean) => void;
  setModel: (model: string, providerId?: string) => void;
  setEffort: (effort: Effort) => void;
  /**
   * `/compact` (§5.11) и `/clear` — про идущий разговор, и на домашнем экране
   * §9.1 их нет: меню не показывает строк, которым некуда приложиться.
   */
  compact?: () => void;
  newChat?: () => void;
  /** `/checkpoints` (§7.3, §1.1) — точки этого разговора, а не проекта. */
  openCheckpoints?: () => void;
  /** `/mcp` (§7.3) — серверы этого разговора и состояние каждого. */
  openMcp?: () => void;
  /**
   * `＋` (§7.2): спросить файлы и вернуть пути от корня проекта.
   *
   * Диалог принадлежит оболочке, а корень — экрану: композер один и тот же в
   * сессии и на домашнем экране, и знать про корень ему незачем. Нет
   * действия — нет и кнопки.
   */
  pickFiles?: () => Promise<string[]>;
  /**
   * Сохранить вставленную из буфера картинку (base64) и вернуть её путь.
   * У вставленного пути нет, а вложение — это путь (§7.2). Нет действия —
   * вставка картинки остаётся обычной вставкой, то есть ничем.
   */
  stashImage?: (data: string) => Promise<string>;
  /**
   * Подписаться на файлы, брошенные в окно; пути приходят уже от корня
   * проекта. Возвращает отписку.
   */
  onDropped?: (listener: (paths: string[]) => void) => () => void;
  /**
   * Перечитать список команд проекта. Зовётся, когда строка становится
   * командой: файл, написанный минуту назад, должен быть в меню, а не только
   * в ядре, которое читает каталог на каждом сообщении.
   */
  refreshCommands?: () => void;
  /**
   * `@` (§7.2): спросить ядро о файлах проекта по запросу. Ответ приходит в
   * ленту, а не сюда. Нет действия — на экране нет сессии, и меню не открывается.
   */
  searchFiles?: (query: string) => void;
  /**
   * `отменить` у чипа очереди (§7.4): убрать сообщение, которое ещё не влито в
   * ход. Нет действия — кнопки нет, чип только показывает, что ждёт.
   */
  cancelQueued?: (id: string) => void;
};

/** Что показывает и переключает нижний ряд §7.1. */
export type ComposerSettings = {
  mode: Mode;
  provider?: string;
  model: string;
  effort: Effort;
  bypass: boolean;
};

export function Composer({
  settings,
  busy,
  ring,
  usage,
  models,
  commands,
  queued,
  found,
  actions,
  disabled = false,
  draftKey,
}: {
  settings: ComposerSettings;
  busy: boolean;
  ring: RingState;
  usage: UsageSnapshot | null;
  models: ModelInfo[];
  /** Слэш-команды проекта (§7.3) — вторая половина меню над полем. */
  commands: SlashCommand[];
  /** Сообщения, которые ждут очереди ядра (§7.4), по порядку. */
  queued: QueuedMessage[];
  /** Последний ответ `@`-поиска (§7.2); показывается, только если запрос тот же. */
  found: Feed["found"];
  actions: ComposerActions;
  /**
   * Отправлять некуда: на домашнем экране (§9.1) — пока не выбран проект.
   * Поле молчит, а не принимает текст, которому не во что уехать.
   */
  disabled?: boolean;
  /** Черновик запоминается на сессию и переживает перезапуск (§7.4). */
  draftKey: string;
}) {
  const [text, setText] = useState(() => localStorage.getItem(draftKey) ?? "");
  const [menu, setMenu] = useState<"mode" | "model" | "effort" | null>(null);
  const [slashAt, setSlashAt] = useState(0);
  /** Приложенные файлы §7.2 — чипы над полем, уезжают с сообщением. */
  const [attached, setAttached] = useState<string[]>([]);
  /** Вложение прикладывается один раз, сколько бы раз его ни бросили. */
  const attach = (paths: string[]) =>
    setAttached((now) => [...now, ...paths.filter((p) => !now.includes(p))]);

  const onDropped = actions.onDropped;
  useEffect(() => {
    if (onDropped === undefined || disabled) return;
    return onDropped(attach);
  }, [onDropped, disabled]);

  /** Картинки из буфера — во вложения; остальное вставляется как обычно. */
  function pasteImages(event: ClipboardEvent<HTMLTextAreaElement>) {
    const stash = actions.stashImage;
    if (stash === undefined) return;
    const images = Array.from(event.clipboardData.files).filter((f) =>
      f.type.startsWith("image/"),
    );
    if (images.length === 0) return;
    // Иначе в поле ляжет имя файла или ничего: текста в таком буфере нет.
    event.preventDefault();
    for (const image of images) {
      void encoded(image)
        .then(stash)
        .then(
          (path) => attach([path]),
          // Что не сохранилось, то не приложено: чипа нет, и это видно.
          () => {},
        );
    }
  }
  const field = useRef<HTMLTextAreaElement>(null);

  // Что известно о выбранной модели: имя для чипа и умеет ли она усилие
  // (§7.1). `undefined` — модели нет в списке провайдера, и это не
  // то же самое, что «не умеет».
  const activeModel =
    models.find(
      (m) =>
        m.id === settings.model &&
        (!settings.provider || !m.providerId || m.providerId === settings.provider),
    ) ?? models.find((m) => m.id === settings.model);

  // Встроенные команды §7.3 — только те, за которыми в этом экране есть
  // действие: на домашнем экране разговора ещё нет, сжимать и начинать заново
  // нечего.
  const builtins: Builtin[] = ["model"];
  if (actions.compact) builtins.push("compact");
  if (actions.newChat) builtins.push("clear");
  if (actions.openCheckpoints) builtins.push("checkpoints");
  if (actions.openMcp) builtins.push("mcp");

  const slash = menuFor(text, commands, builtins);
  const slashIndex = slash === null ? 0 : Math.min(slashAt, slash.length - 1);

  /** Где каретка: `@` читает текст до неё (§7.2). */
  const [caret, setCaret] = useState(0);
  /** Куда поставить каретку, когда поле получит текст, который её сдвигает. */
  const pendingCaret = useRef<number | null>(null);
  const mentions = useMentions({
    text,
    caret,
    found,
    search: actions.searchFiles,
    onPick: (path, mention) => pickFile(path, mention),
  });

  useEffect(() => {
    if (text) localStorage.setItem(draftKey, text);
    else localStorage.removeItem(draftKey);
  }, [text, draftKey]);

  // Список команд перечитывается, когда строка стала командой, а не на каждом
  // нажатии: каталог проекта могли пополнить, пока окно открыто.
  const typing = text.startsWith("/");
  const refresh = actions.refreshCommands;
  useEffect(() => {
    if (typing) refresh?.();
  }, [typing, refresh]);

  // Автовысота до двенадцати строк, дальше внутренний скролл (§7).
  useEffect(() => {
    const node = field.current;
    if (!node) return;
    node.style.height = "auto";
    node.style.height = `${node.scrollHeight}px`;
    const pending = pendingCaret.current;
    if (pending !== null) {
      pendingCaret.current = null;
      node.setSelectionRange(pending, pending);
    }
  }, [text]);

  /**
   * Опустошить поле после отправки или встроенной команды.
   *
   * Черновик стирается здесь же, а не эффектом выше: и `/clear`, и первое
   * сообщение с домашнего экрана уводят окно на другой экран, а эффект
   * размонтированного композера уже не выполнится, и отправленное осталось бы
   * лежать черновиком (§7.4).
   */
  function clear() {
    setText("");
    setAttached([]);
    localStorage.removeItem(draftKey);
  }

  /** Встроенное делает интерфейс; в ядро уезжает команда, а не текст. */
  function run(builtin: Builtin) {
    if (builtin === "compact") actions.compact?.();
    if (builtin === "clear") actions.newChat?.();
    if (builtin === "checkpoints") actions.openCheckpoints?.();
    if (builtin === "mcp") actions.openMcp?.();
    if (builtin === "model") setMenu("model");
    clear();
  }

  /**
   * Взять подсвеченную строку меню.
   *
   * Встроенная выполняется — аргументов она не берёт. Проектная дописывается
   * в поле: за именем может идти всё, что человек хочет ей передать.
   */
  function pick(item: SlashItem) {
    setSlashAt(0);
    if (item.builtin !== null) {
      run(item.builtin);
      return;
    }
    setText(`/${item.name} `);
  }

  /**
   * Взять файл из меню `@` (§7.2): `@запрос` уходит из поля, файл становится
   * чипом над ним — тем же, что приложенный через `＋`, и уедет с сообщением.
   */
  function pickFile(path: string, mention: Mention) {
    const next = withoutMention(text, mention, caret);
    setText(next.text);
    setCaret(next.caret);
    pendingCaret.current = next.caret;
    attach([path]);
  }

  function send() {
    if (disabled || !text.trim()) return;
    // §7.3: встроенная команда не уезжает модели текстом. Проектная уезжает —
    // её разворачивает ядро, у которого и лежат файлы.
    const builtin = builtinFor(text, builtins);
    if (builtin !== null) {
      run(builtin);
      return;
    }
    actions.send(text, attached);
    clear();
  }

  return (
    <div className="composer-wrap" onClick={() => setMenu(null)}>
      <div className="composer" onClick={(e) => e.stopPropagation()}>
        {queued.map((message) => (
          <QueuedChip
            key={message.id}
            text={message.text}
            onCancel={
              actions.cancelQueued && (() => actions.cancelQueued?.(message.id))
            }
          />
        ))}

        {attached.length > 0 && (
          <div className="attachments">
            {attached.map((path) => (
              <Attachment
                key={path}
                path={path}
                onRemove={() => setAttached((now) => now.filter((p) => p !== path))}
              />
            ))}
          </div>
        )}

        <div className="menu-anchor">
          {slash !== null && (
            <SlashMenu items={slash} at={slashIndex} onPick={pick} />
          )}
          {mentions.open !== null && (
            <MentionMenu paths={mentions.paths} at={mentions.at} onPick={mentions.pick} />
          )}

          <textarea
            ref={field}
            rows={2}
            value={text}
            disabled={disabled}
            placeholder={t("composer.placeholder")}
            onChange={(e) => {
              setText(e.target.value);
              setCaret(e.target.selectionStart);
            }}
            onSelect={(e) => setCaret(e.currentTarget.selectionStart)}
            onPaste={pasteImages}
            onKeyDown={(e) => {
              // Пока открыто меню команд §7.3, стрелки, `⇥` и `⏎` принадлежат
              // ему. `⇧⇥` не трогаем: это переключатель режима §13.
              if (slash !== null) {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setSlashAt((now) => (now + 1) % slash.length);
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setSlashAt((now) => (now - 1 + slash.length) % slash.length);
                  return;
                }
                if ((e.key === "Tab" || e.key === "Enter") && !e.shiftKey) {
                  e.preventDefault();
                  pick(slash[slashIndex]);
                  return;
                }
              }

              // `@` (§7.2): пока открыто меню файлов, стрелки, ⏎ и ⇥ его, а
              // Esc закрывает его, не останавливая агента.
              if (mentions.keyDown(e)) return;

              // Пока висит запрос разрешения, цифры принадлежат ему (§5.7),
              // а поле всё равно остаётся живым.
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                send();
              }
            }}
          />
        </div>

        <div className="composer-row">
          <div className="menu-anchor">
            <button
              className={settings.bypass ? "chip chip-danger" : "chip"}
              onClick={() => setMenu(menu === "mode" ? null : "mode")}
            >
              {t(MODE_KEY[settings.mode])}
            </button>
            {menu === "mode" && (
              <ModeMenu
                settings={settings}
                actions={actions}
                onClose={() => setMenu(null)}
              />
            )}
          </div>

          {/* §7.1: `＋` вложения сразу за чипом режима. Нет действия — нет и
              кнопки: экран без корня проекта прикладывать нечего. */}
          {actions.pickFiles && (
            <button
              className="chip chip-icon"
              aria-label={t("composer.attach")}
              title={t("composer.attach")}
              onClick={() => {
                void actions.pickFiles?.().then(
                  (picked) =>
                    setAttached((now) => [
                      ...now,
                      ...picked.filter((path) => !now.includes(path)),
                    ]),
                  // Диалог, который не открылся, ничего не приложил: показывать
                  // тут нечего, а ошибку экрана заводить ради этого — много.
                  () => {},
                );
              }}
            >
              <Plus size={14} weight="bold" />
            </button>
          )}

          <div className="composer-gap" />

          <ModelMenu
            models={models}
            active={activeModel}
            model={settings.model}
            provider={settings.provider}
            open={menu === "model"}
            onToggle={() => setMenu(menu === "model" ? null : "model")}
            onPick={(model) => {
              actions.setModel(model.id, model.providerId);
              setMenu(null);
            }}
          />

          {/* §7.1 ставит усилие рядом с моделью, но у модели без reasoning
              выбирать нечего: ядро поле не отправляет вовсе, потому что
              `reasoning_effort` там, где его не ждут, — это 400. Чип, который
              переключается и ничего не меняет, врёт про состояние (§0).

              Прячется только по явному «нет»: модели, которой нет в списке,
              усилие могло быть и доступно, и спрятать существующий уровень
              молча хуже, чем показать лишний чип. */}
          {activeModel?.supportsEffort !== false && (
            <EffortMenu
              effort={settings.effort}
              open={menu === "effort"}
              onToggle={() => setMenu(menu === "effort" ? null : "effort")}
              onPick={(effort) => {
                actions.setEffort(effort);
                setMenu(null);
              }}
            />
          )}

          <StatusRing state={ring} usage={usage} />

          {busy ? (
            <button
              className="send send-stop"
              onClick={actions.stop}
              aria-label={t("composer.stop")}
            >
              <Stop size={14} weight="fill" />
            </button>
          ) : (
            <button
              className="send"
              onClick={send}
              disabled={disabled || !text.trim()}
              aria-label={t("composer.send")}
            >
              <ArrowUp size={16} weight="bold" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

/** Файл как base64 без префикса `data:` — в том виде, в каком его ждёт ядро. */
function encoded(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error);
    reader.onload = () => {
      const url = typeof reader.result === "string" ? reader.result : "";
      resolve(url.slice(url.indexOf(",") + 1));
    };
    reader.readAsDataURL(file);
  });
}
