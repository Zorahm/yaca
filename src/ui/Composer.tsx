/**
 * Композер §7 и его нижний ряд §7.1.
 *
 * Поле не блокируется, пока агент работает (§7.4): отправленное встаёт в
 * очередь ядра и показывается чипом над полем. Отменить очередь пока нельзя —
 * такой команды у ядра нет, и ссылка «отменить», которая ничего не отменяет,
 * была бы худшим из двух вариантов (§0). Чип говорит только то, что правда.
 *
 * Настройки нижнего ряда приходят пропсом, а не берутся из `SessionState`:
 * §9.1 ставит тот же композер на домашний экран, где сессии ещё нет, а
 * выбранное там уезжает в неё вместе с первым сообщением.
 *
 * Чего здесь нет из §7.1: кнопки диктовки. Распознавания речи нет, а кнопка,
 * молча ничего не делающая, это интерфейс, врущий про состояние. `＋` есть:
 * диалог файлов у оболочки нашёлся. `@` (§7.2) — нет: ему нужен поиск по
 * проекту, которого ядро наружу не отдаёт.
 *
 * `/` в начале строки открывает меню команд §7.3 (`./slash`). Меню не
 * состояние, а функция от текста поля: `⇥` и `⏎` берут подсвеченное,
 * пробел закрывает — дальше идут аргументы.
 */

import { ArrowUp, Check, Plus, Stop } from "@phosphor-icons/react";
import { useEffect, useRef, useState, type ClipboardEvent } from "react";

import type { Effort, Mode, ModelInfo, SlashCommand, UsageSnapshot } from "../core/client";
import { t, type Key } from "../i18n";
import { EffortMenu } from "./EffortMenu";
import { ModelMenu } from "./ModelMenu";
import { SlashMenu } from "./SlashMenu";
import { Attachment } from "./Attachment";
import { builtinFor, menuFor, type Builtin, type SlashItem } from "./slash";
import { StatusRing, type RingState } from "./StatusRing";

export const MODES: Mode[] = ["plan", "manual", "auto", "edits"];

const MODE_KEY: Record<Mode, Key> = {
  plan: "mode.plan",
  manual: "mode.manual",
  auto: "mode.auto",
  edits: "mode.edits",
};
const MODE_NOTE: Record<Mode, Key> = {
  plan: "mode.plan_note",
  manual: "mode.manual_note",
  auto: "mode.auto_note",
  edits: "mode.edits_note",
};

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
  queued: string | null;
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

  const slash = menuFor(text, commands, builtins);
  const slashIndex = slash === null ? 0 : Math.min(slashAt, slash.length - 1);

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
        {queued !== null && (
          <div className="queued">
            <span>{t("composer.queued")}</span>
            <span className="mono-target">{queued}</span>
          </div>
        )}

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

          <textarea
            ref={field}
            rows={2}
            value={text}
            disabled={disabled}
            placeholder={t("composer.placeholder")}
            onChange={(e) => setText(e.target.value)}
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

/**
 * §8: четыре взаимоисключающих режима одним списком; обход — флаг поверх.
 *
 * План стоит в этом же списке, а не отдельной осью: записывающих тулов в нём
 * нет, значит доверие в этот момент ничего не решает, и два контрола показывали
 * бы выбор, которого не существует.
 */
function ModeMenu({
  settings,
  actions,
  onClose,
}: {
  settings: ComposerSettings;
  actions: ComposerActions;
  onClose: () => void;
}) {
  const setBypass = actions.setBypass;

  // Цифры принадлежат открытому меню, а не запросу разрешения (§5.7): пока
  // список на экране и показывает свои `1`–`4`, они обязаны делать написанное.
  // Отсюда фаза перехвата — слушатель запроса висит на всплытии.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const at = Number(event.key) - 1;
      if (!Number.isInteger(at) || at < 0 || at >= MODES.length) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      actions.setMode(MODES[at]);
      onClose();
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [actions, onClose]);

  return (
    <div className="menu menu-notes">
      <div className="menu-item" style={{ color: "var(--ink-muted)" }}>
        {t("mode.title")}
      </div>
      {MODES.map((mode, index) => (
        <button
          className="menu-item"
          key={mode}
          onClick={() => {
            actions.setMode(mode);
            onClose();
          }}
        >
          <span className="menu-text">
            <span>{t(MODE_KEY[mode])}</span>
            <span className="menu-note">{t(MODE_NOTE[mode])}</span>
          </span>
          {settings.mode === mode && <span className="menu-mark"><Check size={13} weight="bold" /></span>}
          <span className="keycap">{index + 1}</span>
        </button>
      ))}

      {setBypass && (
        <>
          <div className="menu-sep" />

          <button
            className="menu-item"
            onClick={() => {
              // Включение требует подтверждения; инварианты не отключаются ничем.
              if (settings.bypass || window.confirm(t("mode.bypass_confirm"))) {
                setBypass(!settings.bypass);
              }
              onClose();
            }}
          >
            <span className={settings.bypass ? "chip-danger" : undefined}>
              {t("mode.bypass")}
            </span>
            <span className="menu-note" />
            <span>
              {settings.bypass ? t("mode.bypass_off") : t("mode.bypass_on")}
            </span>
          </button>
        </>
      )}
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
