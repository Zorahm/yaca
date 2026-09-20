/**
 * Дифф §6.3. Одинаков в раскрытой строке действия и в панели файла.
 *
 * Ханки, номера строк и пословные диапазоны считает ядро (§12): результат
 * одинаков везде, где дифф показывается, и разбирать текст в вебвью параллельно
 * дельтам всё равно не выйдет.
 *
 * Дифф показывается целиком: раскрытие, за которым нужен второй клик, — это
 * не раскрытие (§16.34). Длинный дифф рисуется окном (§12), а не режется:
 * скролл у самого блока, и за его краями строк в разметке просто нет.
 *
 * Чего здесь нет — подсветки синтаксиса. §6.3 называет её тем, что надо
 * добавить сверх прототипа; половина подсветки хуже её отсутствия, а целая —
 * это отдельный разбор языка в воркере (§12).
 */

import { useEffect, useMemo, useRef, useState, type ReactElement, type ReactNode } from "react";

import type { DiffLine, FileDiff, Span } from "../core/client";
import { t, tp } from "../i18n";
import { windowOf } from "./window";

/**
 * Короче этого дифф рисуется целиком: спейсеры и слушатель прокрутки на
 * дифф в десять строк — расход без выигрыша.
 */
const WINDOW_FROM = 200;

/** Сколько строк держать нарисованными за краями окна. */
const OVERSCAN = 8;

export function Diff({
  diff,
  soft = false,
  gaps = false,
}: {
  diff: FileDiff;
  /** В строке действия подложка мягче, чем в панели (§2). */
  soft?: boolean;
  /** Свёрнутые промежутки «⋯ N строк» между ханками — панель файла (§6.4). */
  gaps?: boolean;
}) {
  const items = useMemo(() => rows(diff, gaps), [diff, gaps]);

  const box = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ scrollTop: 0, height: 0, row: 0 });
  const windowed = items.length > WINDOW_FROM;

  useEffect(() => {
    const node = box.current;
    if (!node || !windowed) return;

    // Высота строки берётся из токена, а не из константы: §2 обещает 20 px в
    // компактном режиме, и окно, посчитанное по 24, промахнётся мимо экрана.
    const row = parseFloat(
      getComputedStyle(document.documentElement).getPropertyValue("--diff-line"),
    );

    const measure = () =>
      setView({ scrollTop: node.scrollTop, height: node.clientHeight, row });

    measure();
    node.addEventListener("scroll", measure, { passive: true });
    const resize = new ResizeObserver(measure);
    resize.observe(node);
    return () => {
      node.removeEventListener("scroll", measure);
      resize.disconnect();
    };
  }, [windowed]);

  const slice = windowed
    ? windowOf(items.length, view.row, view.scrollTop, view.height, OVERSCAN)
    : { start: 0, end: items.length, padTop: 0, padBottom: 0 };

  return (
    <div className={soft ? "diff diff-soft" : "diff"} ref={box}>
      {slice.padTop > 0 && <div style={{ height: slice.padTop }} />}
      {items.slice(slice.start, slice.end)}
      {slice.padBottom > 0 && <div style={{ height: slice.padBottom }} />}
      {diff.truncated && <div className="diff-more">{t("row.diff_truncated")}</div>}
    </div>
  );
}

/**
 * Дифф одним плоским списком строк одинаковой высоты.
 *
 * Плоским — потому что окно считает по индексу, и вложенность ханков ему
 * мешала бы; одинаковой высоты — потому что заголовок ханка и промежуток
 * лежат на той же сетке, что строки (§6.3).
 */
function rows(diff: FileDiff, gaps: boolean): ReactElement[] {
  const out: ReactElement[] = [];

  for (const [index, hunk] of diff.hunks.entries()) {
    // Промежуток считается от старой стороны: у созданного файла его не
    // бывает, у правки середины — ровно столько, сколько строк между ханками.
    if (gaps) {
      const before = gapBefore(diff.hunks, index);
      if (before > 0) {
        out.push(
          <div className="diff-gap" key={`g${index}`}>
            ⋯ {tp("file.gap", before)}
          </div>,
        );
      }
    }
    out.push(
      <div className="hunk" key={`h${index}`}>
        {`@@ -${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines} @@`}
      </div>,
    );
    for (const [at, line] of hunk.lines.entries()) {
      out.push(<Line key={`h${index}l${at}`} line={line} />);
    }
  }

  return out;
}

/** Сколько строк старой стороны спрятано перед ханком (§6.4). */
function gapBefore(hunks: FileDiff["hunks"], index: number): number {
  if (index === 0) return hunks[0].oldStart - 1;
  const before = hunks[index - 1];
  return hunks[index].oldStart - (before.oldStart + before.oldLines);
}

function Line({ line }: { line: DiffLine }) {
  const kind =
    line.kind === "add" ? "diff-add" : line.kind === "del" ? "diff-del" : "diff-ctx";
  const sign = line.kind === "add" ? "+" : line.kind === "del" ? "−" : " ";
  // Жёлоб показывает ту сторону, к которой строка относится (§6.3).
  const number = line.kind === "del" ? line.oldLine : line.newLine;

  return (
    <div className={`diff-line ${kind}`}>
      <span className="diff-gutter">{number ?? ""}</span>
      <span className="diff-sign">{sign}</span>
      <span className="diff-text">{words(line.text, line.spans)}</span>
    </div>
  );
}

/**
 * Пословная подсветка внутри изменённой строки (§6.3).
 *
 * Диапазоны ядро считает в символах, а не в байтах, поэтому строку режем
 * массивом кодовых точек: `slice` по индексам сломал бы суррогатные пары.
 */
function words(text: string, spans: Span[]): ReactNode {
  if (spans.length === 0) return text;

  const chars = Array.from(text);
  const out: ReactNode[] = [];
  let at = 0;

  for (const [index, span] of spans.entries()) {
    if (span.start > at) out.push(chars.slice(at, span.start).join(""));
    out.push(
      <span className="word" key={index}>
        {chars.slice(span.start, span.end).join("")}
      </span>,
    );
    at = span.end;
  }
  if (at < chars.length) out.push(chars.slice(at).join(""));
  return out;
}
