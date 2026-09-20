/**
 * Группа действий (§5.4) — ключевой компонент ленты.
 *
 * Свёрнутая группа это строка в общем потоке, а не карточка: тот же кегль, что
 * у текста агента, цвет приглушённый, ни рамки, ни фона (§16.16). Лента от
 * этого читается разговором, в который вставлены строчки отчёта, а не
 * лестницей карточек.
 *
 * Пока группа выполняется, она раскрыта. По завершении сворачивается сама —
 * но только если её не раскрыл человек: своё решение важнее нашего.
 */

import { CaretDown, CaretRight } from "@phosphor-icons/react";
import { useState } from "react";

import type { ActionRow } from "../core/feed";
import { ActionRowView } from "./ActionRow";
import { summary } from "./captions";

export function ActionGroup({
  rows,
  onOpenFile,
}: {
  rows: ActionRow[];
  onOpenFile: (path: string) => void;
}) {
  const running = rows.some((r) => r.state === "running");
  const [override, setOverride] = useState<boolean | null>(null);
  const open = override ?? running;

  const head = summary(rows);

  return (
    <div>
      <button
        className="group-head"
        aria-expanded={open}
        onClick={() => setOverride(!open)}
      >
        {/* Шапка собирается и на ходу: §5.4 просит её пересобирать по
            завершении каждого действия, а не ждать конца группы. */}
        <span className="group-title">
          {head.parts.map((part, index) => (
            <span key={index}>
              {index > 0 && ", "}
              {part.text}
              {part.failedNote && (
                <span className="group-failed">{part.failedNote}</span>
              )}
            </span>
          ))}
        </span>
        {(head.added > 0 || head.removed > 0) && (
          <span className="counts">
            <span className="count-add">+{head.added}</span>{" "}
            <span className="count-del">−{head.removed}</span>
          </span>
        )}
        <span className="caret">{open ? <CaretDown size={12} /> : <CaretRight size={12} />}</span>
      </button>

      {open && (
        // §5.4: группа не скроллится. Раскрытая показывает все свои строки, а
        // высоту ограничивает то содержимое, которое умеет скроллиться само,
        // — дифф и вывод в раскрытой строке (§5.5.1).
        <div className="group-list">
          {rows.map((row) => (
            <ActionRowView
              key={row.callId}
              row={row}
              onOpenFile={onOpenFile}
            />
          ))}
        </div>
      )}
    </div>
  );
}
