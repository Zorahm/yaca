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
 * Синтаксис подсвечивается построчно (§6.3): грамматика выбирается по
 * расширению файла и подгружается лениво из бандла (`languages.ts`), пока не
 * приехала — строка рендерится обычным текстом. Подсветка не режет строку на
 * разные высоты: окно §12 по-прежнему считает по токену `--diff-line`.
 */

import { useEffect, useMemo, useRef, useState, type ReactElement, type ReactNode } from "react";

import type { DiffLine, FileDiff, Span } from "../core/client";
import { t, tp } from "../i18n";
import { runsOf } from "./codeText";
import { utf16Spans, between, plainLine } from "./highlight";
import { languageOf } from "./languages";
import { useHighlight, type LineHighlighter } from "./useHighlight";
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
  path,
}: {
  diff: FileDiff;
  /** В строке действия подложка мягче, чем в панели (§2). */
  soft?: boolean;
  /** Свёрнутые промежутки «⋯ N строк» между ханками — панель файла (§6.4). */
  gaps?: boolean;
  /** Путь файла: из него выбирается грамматика подсветки (§6.3). */
  path?: string;
}) {
  const language = useMemo(() => (path === undefined ? null : languageOf(path)), [path]);
  const highlight = useHighlight(language);
  const items = useMemo(
    () => rows(diff, gaps, highlight),
    [diff, gaps, highlight],
  );

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
 *
 * Грамматика резолвится один раз на весь дифф, а не на строку: у hook'а одна
 * подписка, и строки не перерисовывают друг друга наперегонки.
 */
function rows(
  diff: FileDiff,
  gaps: boolean,
  highlight: LineHighlighter | null,
): ReactElement[] {
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
      out.push(<Line key={`h${index}l${at}`} line={line} highlight={highlight} />);
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

function Line({
  line,
  highlight,
}: {
  line: DiffLine;
  /** Пока грамматика не приехала — `null`, и строка рендерится без подсветки. */
  highlight: LineHighlighter | null;
}) {
  const kind =
    line.kind === "add" ? "diff-add" : line.kind === "del" ? "diff-del" : "diff-ctx";
  const sign = line.kind === "add" ? "+" : line.kind === "del" ? "−" : " ";
  // Жёлоб показывает ту сторону, к которой строка относится (§6.3).
  const number = line.kind === "del" ? line.oldLine : line.newLine;

  return (
    <div className={`diff-line ${kind}`}>
      <span className="diff-gutter">{number ?? ""}</span>
      <span className="diff-sign">{sign}</span>
      <span className="diff-text">{styled(line.text, line.spans, highlight)}</span>
    </div>
  );
}

/**
 * Текст строки: синтаксическая подсветка Lezer (§6.3) под пословной.
 *
 * Пословные диапазоны ядра считаются в кодточках, а не в байт-единицах:
 * их границы переводятся в UTF-16, чтобы сходиться с диапазонами Lezer —
 * иначе суррогатная пара разрезала бы и слово, и токен.
 */
function styled(text: string, spans: Span[], highlight: LineHighlighter | null): ReactNode {
  const pieces = highlight === null ? plainLine(text) : highlight(text);
  const bounds = utf16Spans(text, spans);

  if (bounds.length === 0) return runsOf(text, pieces);

  const out: ReactNode[] = [];
  let at = 0;
  for (const [index, bound] of bounds.entries()) {
    if (bound.from > at) {
      out.push(runsOf(text.slice(at, bound.from), between(pieces, at, bound.from)));
    }
    out.push(
      <span className="word" key={index}>
        {runsOf(
          text.slice(bound.from, bound.to),
          between(pieces, bound.from, bound.to),
        )}
      </span>,
    );
    at = bound.to;
  }
  if (at < text.length) {
    out.push(runsOf(text.slice(at), between(pieces, at, text.length)));
  }
  return out;
}
