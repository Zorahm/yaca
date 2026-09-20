/**
 * Клавиатура §13.
 *
 * Здесь только те сочетания, за которыми есть действие. `⌘K`, `⌘N`, `⌘F` и
 * `⌘,` ведут в палитру команд, список сессий, поиск и настройки — их в этом
 * срезе нет, и сочетание, которое ничего не делает, ничем не лучше кнопки,
 * которая ничего не делает.
 *
 * `Esc Esc` откатывается к предыдущему сообщению: находит последнюю точку,
 * снятую перед сообщением, и показывает список файлов — сам откат подтверждает
 * человек (§5.8). Пока открыта выдвижная панель §1.1, первое `Esc` принадлежит
 * ей: её закрывает Radix, и останавливать агента или засчитывать откат оно
 * не должно.
 */

import { useEffect, useRef } from "react";

/** Сколько времени второй `Esc` считается вторым (§13). */
const DOUBLE_ESC_MS = 600;

export type Shortcuts = {
  stop: () => void;
  rollbackToPreviousMessage: () => void;
  nextMode: () => void;
  /** Пока висит запрос разрешения, клавиши принадлежат ему (§5.7). */
  suspended: boolean;
  /** Открыта выдвижная панель §1.1. */
  panelOpen: boolean;
  /** `⌘⇧O` — закрыть выдвижную панель (§13). */
  closePanel: () => void;
  /** `⌥←` / `⌥→` — соседний файл в панели файла (§13). */
  prevFile: () => void;
  nextFile: () => void;
};

export function useShortcuts(shortcuts: Shortcuts): void {
  const lastEsc = useRef(0);
  const current = useRef(shortcuts);
  current.current = shortcuts;

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const s = current.current;
      if (s.suspended) return;

      const meta = event.ctrlKey || event.metaKey;

      if (event.key === "Escape") {
        // Первое Esc при открытой панели закрывает её — это делает Radix.
        if (s.panelOpen) return;
        const now = Date.now();
        if (now - lastEsc.current < DOUBLE_ESC_MS) {
          lastEsc.current = 0;
          event.preventDefault();
          s.rollbackToPreviousMessage();
          return;
        }
        lastEsc.current = now;
        s.stop();
        return;
      }

      if (s.panelOpen && meta && event.shiftKey && event.key.toUpperCase() === "O") {
        event.preventDefault();
        s.closePanel();
        return;
      }

      if (s.panelOpen && event.altKey && !meta) {
        if (event.key === "ArrowLeft") {
          event.preventDefault();
          s.prevFile();
          return;
        }
        if (event.key === "ArrowRight") {
          event.preventDefault();
          s.nextFile();
          return;
        }
      }

      // Голый `⇥` не перехватывается: осей стало одна, и отбирать у поля
      // обычный переход фокуса больше не за что.
      if (event.key === "Tab" && event.shiftKey && inComposer(event.target)) {
        event.preventDefault();
        s.nextMode();
      }
    }

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

/** `⇧⇥` переключает режим только из композера: в списке это обход фокуса. */
function inComposer(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.tagName === "TEXTAREA";
}
