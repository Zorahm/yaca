/**
 * Строка действия и её раскрытие.
 *
 * Раскрытие происходит внутри группы: строка растёт вниз, соседние остаются на
 * местах. Раскрыта по умолчанию только выполняющаяся — список из пятнадцати
 * действий читается как список, а не как простыня.
 *
 * Строки `Task` здесь нет: подагенты стоят в ленте плашкой (`AgentsPlate`), а
 * их действия — этими же строками в панели агентов.
 */

import { CaretDown, CaretRight } from "@phosphor-icons/react";
import { useState } from "react";

import type { ActionRow } from "../core/feed";
import { t, tp } from "../i18n";
import { caption, fileChildren, isFileRow, rowCounts, rowFiles } from "./captions";
import { CodeBlock } from "./CodeBlock";
import { Diff } from "./Diff";
import { plainPath } from "./format";
import { parseRead } from "./readOutput";
import { RowSign } from "./RowSign";
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
  // What the model asked and what came back must not look alike: the request
  // (a command, a pattern, a path) sits in a framed box, and under it the
  // tool's answer is plain text. `Bash` adds a prompt. A row with a diff keeps
  // its path as a plain line: the diff is its own answer.
  const prompt = row.tool === "Bash";
  // A row about a file is laid out around the file's name: the counters and
  // the caret stand right after it, and the body is the path and, framed, the
  // file's lines — never a command box.
  const file = isFileRow(row);
  // What was read is drawn like a diff — numbers in a gutter, text in the
  // file's colours — not as the padded listing the tool wrote for the model.
  const read = row.tool === "Read" && row.state === "done" ? parseRead(row.output) : null;
  // A rename shows its files, each a row with its own diff, in place of the
  // list the tool wrote for the model: the rows are that list.
  const fileRows = fileChildren(row);
  const boxed = !file;

  return (
    <div className={open ? "row row-open" : "row"}>
      <button
        className={file ? "row-head row-head-file" : "row-head"}
        aria-expanded={open}
        onClick={() => setOverride(!open)}
      >
        <RowSign row={row} />
        {/* Клик по имени файла открывает панель файла, клик мимо имени —
            раскрывает строку. Имена у тулов без факта не кликабельны. */}
        <span className="row-caption">{caption(row, onOpenFile)}</span>
        {counts && (
          <span className="counts counts-pill">
            <span className="count-add">+{counts.added}</span>
            <span className="count-del">−{counts.removed}</span>
          </span>
        )}
        {files !== null && <span className="counts">{tp("row.files", files)}</span>}
        <span className="caret">{open ? <CaretDown size={12} /> : <CaretRight size={12} />}</span>
      </button>

      {open && (
        <div className={boxed ? "row-body row-body-cmd" : "row-body row-body-file"}>
          {/* Первая строка тела — полный путь или полная команда. */}
          {row.target && !boxed && (
            <div className="row-target">{plainPath(row.target)}</div>
          )}
          {row.target && boxed && (
            <pre className="cmd-box">
              {prompt && <span className="cmd-prompt">$ </span>}
              {prompt ? row.target : plainPath(row.target)}
            </pre>
          )}

          {/* То, на что модель посмотрела, человек видит тоже: «прочитал
              скриншот» без скриншота — отчёт, который нечем проверить. */}
          {row.tool === "ReadImage" && row.target && row.state === "done" && (
            <Thumb path={row.target} />
          )}
          {/* And what a tool handed back as images — a page's screenshot:
              the files the core kept of them (`ToolMeta.pictures`). */}
          {row.state === "done" &&
            row.meta?.pictures?.map((path) => <Thumb key={path} path={path} />)}

          {fileRows.length > 0 ? (
            <div className="group-list">
              {fileRows.map((child) => (
                <ActionRowView key={child.callId} row={child} onOpenFile={onOpenFile} />
              ))}
            </div>
          ) : row.diff ? (
            <Diff diff={row.diff} soft path={row.fact?.path ?? row.target ?? undefined} />
          ) : read ? (
            <>
              <Diff
                body={{ lines: read.lines, truncated: false }}
                soft
                path={row.target ?? undefined}
              />
              {read.note && <div className="row-target row-dim row-note">{read.note}</div>}
            </>
          ) : row.output.trim() ? (
            // What a file holds is framed like a diff; what a command printed
            // is plain text under its box.
            <CodeBlock text={row.output} error={row.state === "failed"} plain={boxed} />
          ) : (
            <div className="row-target row-dim">{t("row.no_output")}</div>
          )}

          {/* Причина, по которой действие не состоялось, — целиком: в подписи
              она обрезана до строки, а чинить по обрезку нечего. */}
          {row.message && row.state !== "denied" && row.state !== "stopped" && (
            <div className="row-target row-dim">{row.message}</div>
          )}
        </div>
      )}
    </div>
  );
}
