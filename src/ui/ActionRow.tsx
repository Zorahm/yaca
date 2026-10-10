/**
 * Строка действия и её раскрытие (§5.5, §5.5.1).
 *
 * Раскрытие происходит внутри группы: строка растёт вниз, соседние остаются на
 * местах. Раскрыта по умолчанию только выполняющаяся — список из пятнадцати
 * действий читается как список, а не как простыня.
 *
 * У строки `Task` в раскрытии сперва идут строки подагента — те же самые, с
 * тем же раскрытием в дифф и вывод, — а под ними отчёт, который он вернул.
 */

import { CaretDown, CaretRight, Circle } from "@phosphor-icons/react";
import { useState } from "react";

import type { ActionRow } from "../core/feed";
import { t, tp } from "../i18n";
import { caption, rowCounts, rowFiles } from "./captions";
import { CodeBlock } from "./CodeBlock";
import { Diff } from "./Diff";
import { plainPath } from "./format";
import { Thumb } from "./Thumb";

export function ActionRowView({
  row,
  onOpenFile,
}: {
  row: ActionRow;
  onOpenFile: (path: string) => void;
}) {
  const running = row.state === "running";
  const [override, setOverride] = useState<boolean | null>(null);
  const open = override ?? running;
  const counts = rowCounts(row);
  const files = rowFiles(row);

  return (
    <div className={open ? "row row-open" : "row"}>
      <button
        className="row-head"
        aria-expanded={open}
        onClick={() => setOverride(!open)}
      >
        {running && <span className="row-dot pulse"><Circle size={7} weight="fill" /></span>}
        {/* §1.1: клик по имени файла открывает панель файла, клик мимо имени —
            раскрывает строку. Имена у тулов без факта не кликабельны. */}
        <span className="row-caption">{caption(row, onOpenFile)}</span>
        {counts && (
          <span className="counts">
            <span className="count-add">+{counts.added}</span>{" "}
            <span className="count-del">−{counts.removed}</span>
          </span>
        )}
        {files !== null && <span className="counts">{tp("row.files", files)}</span>}
        <span className="caret">{open ? <CaretDown size={12} /> : <CaretRight size={12} />}</span>
      </button>

      {open && (
        <div className="row-body">
          {/* Первая строка тела — полный путь или полная команда (§5.5.1). */}
          {row.target && <div className="row-target">{plainPath(row.target)}</div>}

          {/* То, на что модель посмотрела, человек видит тоже: «прочитал
              скриншот» без скриншота — отчёт, который нечем проверить. */}
          {row.tool === "ReadImage" && row.target && row.state === "done" && (
            <Thumb path={row.target} />
          )}

          {row.children.length > 0 && (
            <div className="group-list">
              {row.children.map((child) => (
                <ActionRowView key={child.callId} row={child} onOpenFile={onOpenFile} />
              ))}
            </div>
          )}

          {row.diff ? (
            <Diff diff={row.diff} soft path={row.fact?.path ?? row.target ?? undefined} />
          ) : row.output.trim() ? (
            <CodeBlock text={row.output} error={row.state === "failed"} />
          ) : (
            // Пока подагент работает, отчёта нет, но и «вывода нет» — неправда:
            // его строки выше и есть то, что происходит.
            row.children.length === 0 && (
              <div className="row-target row-dim">{t("row.no_output")}</div>
            )
          )}

          {/* Причина, по которой действие не состоялось, — целиком: в подписи
              она обрезана до строки, а чинить по обрезку нечего. */}
          {row.message && row.state !== "denied" && row.state !== "stopped" && (
            <div className="row-target row-dim">{row.message}</div>
          )}

          {/* Кнопок «принять» и «отклонить» здесь нет: подтверждение
              спрашивается один раз и до записи (§16.37). */}
          {row.fact && (
            <div className="row-actions">
              <button
                className="btn btn-row"
                onClick={() => onOpenFile(row.fact!.path)}
              >
                {t("row.open_file")}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
