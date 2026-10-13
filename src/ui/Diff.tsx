/**
 * Дифф §6.3 в раскрытой строке действия — и тот же блок строк без диффа в
 * панели файла (§6.4), которая показывает файл как он есть.
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

import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactElement,
  type ReactNode,
} from "react";

import type { DiffLine, FileDiff, Span } from "../core/client";
import { t } from "../i18n";
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

/**
 * A file's lines as they are, nothing marked as added or removed: the whole
 * file in the panel (§6.4), or what a `Read` returned in its feed row (with
 * `soft`).
 */
export type FileBody = {
  lines: DiffLine[];
  /** The view stops before the end of the file. */
  truncated: boolean;
  /** Lines the panel's search found, by index in `lines`. */
  hits?: ReadonlySet<number>;
  /** The hit the search stands on: marked stronger and scrolled into view. */
  current?: number | null;
};

export function Diff({
  diff,
  body,
  soft = false,
  path,
}: {
  /** The stream's diff of one call (§5.5.1). Absent when `body` is given. */
  diff?: FileDiff;
  /** The whole file, for the panel. Absent in the feed. */
  body?: FileBody;
  /** В строке действия подложка мягче, чем в панели (§2). */
  soft?: boolean;
  /** Путь файла: из него выбирается грамматика подсветки (§6.3). */
  path?: string;
}) {
  const language = useMemo(() => (path === undefined ? null : languageOf(path)), [path]);
  const highlight = useHighlight(language);
  const items = useMemo(
    () =>
      body
        ? body.lines.map((line, at) => (
            <Line
              key={at}
              line={line}
              highlight={highlight}
              mark={at === body.current ? "now" : body.hits?.has(at) ? "hit" : null}
            />
          ))
        : diff
          ? rows(diff, highlight)
          : [],
    [diff, body, highlight],
  );
  const truncated = body ? body.truncated : (diff?.truncated ?? false);
  // The gutter is as wide as the longest number in it and no wider: a column
  // sized for five digits pushes a ten-line file's text away from its edge.
  const digits = useMemo(() => {
    const shown = body ? body.lines : (diff?.hunks.flatMap((hunk) => hunk.lines) ?? []);
    const top = shown.reduce((max, line) => Math.max(max, line.newLine ?? 0, line.oldLine ?? 0), 0);
    return String(top).length;
  }, [diff, body]);

  const box = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ scrollTop: 0, height: 0, row: 0 });
  const windowed = items.length > WINDOW_FROM;

  useEffect(() => {
    const node = box.current;
    if (!node || !windowed) return;

    const row = rowHeight(node);

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

  // The search steps from hit to hit: the one it stands on comes into view, a
  // third of the way down, so the lines before it are there to read too.
  const current = body?.current ?? null;
  useEffect(() => {
    const node = box.current;
    if (!node || current === null) return;
    const top = current * rowHeight(node);
    if (top < node.scrollTop || top > node.scrollTop + node.clientHeight - rowHeight(node)) {
      node.scrollTop = Math.max(0, top - node.clientHeight / 3);
    }
  }, [current]);

  const slice = windowed
    ? windowOf(items.length, view.row, view.scrollTop, view.height, OVERSCAN)
    : { start: 0, end: items.length, padTop: 0, padBottom: 0 };

  return (
    <div
      // A diff in a feed row wraps long lines like the rest of the tool's
      // output. Windowing needs rows of one height, so a long diff that is
      // windowed keeps single lines and scrolls sideways.
      className={
        soft
          ? `diff diff-soft${windowed ? "" : " diff-wrap"}${body ? " diff-read" : ""}`
          : "diff diff-whole"
      }
      style={{ "--digits": digits } as CSSProperties}
      ref={box}
    >
      {slice.padTop > 0 && <div style={{ height: slice.padTop }} />}
      {items.slice(slice.start, slice.end)}
      {slice.padBottom > 0 && <div style={{ height: slice.padBottom }} />}
      {truncated && (
        <div className="diff-more">{t(body ? "file.truncated" : "row.diff_truncated")}</div>
      )}
    </div>
  );
}

/**
 * Высота строки берётся из токена, а не из константы: §2 обещает 20 px в
 * компактном режиме, и окно, посчитанное по 24, промахнётся мимо экрана.
 * Читается с самого блока: панель файла ставит строкам свою высоту.
 */
function rowHeight(node: HTMLElement): number {
  return parseFloat(getComputedStyle(node).getPropertyValue("--diff-line"));
}

/**
 * Дифф одним плоским списком строк одинаковой высоты.
 *
 * Плоским — потому что окно считает по индексу, и вложенность ханков ему
 * мешала бы; одинаковой высоты — потому что заголовок ханка лежит на той же
 * сетке, что строки (§6.3).
 *
 * Грамматика резолвится один раз на весь дифф, а не на строку: у hook'а одна
 * подписка, и строки не перерисовывают друг друга наперегонки.
 */
function rows(
  diff: FileDiff,
  highlight: LineHighlighter | null,
): ReactElement[] {
  const out: ReactElement[] = [];

  for (const [index, hunk] of diff.hunks.entries()) {
    // In a feed row the `@@ -a,b +c,d @@` header is noise: the reader wants
    // the lines. Hunks are told apart by a bare fold mark on the same grid.
    if (index > 0) {
      out.push(
        <div className="hunk hunk-fold" key={`h${index}`}>
          ⋯
        </div>,
      );
    }
    for (const [at, line] of hunk.lines.entries()) {
      out.push(<Line key={`h${index}l${at}`} line={line} highlight={highlight} />);
    }
  }

  return out;
}

function Line({
  line,
  highlight,
  mark = null,
}: {
  line: DiffLine;
  /** Found by the panel's search; `now` is the hit it stands on. */
  mark?: "hit" | "now" | null;
  /** Пока грамматика не приехала — `null`, и строка рендерится без подсветки. */
  highlight: LineHighlighter | null;
}) {
  const kind =
    line.kind === "add" ? "diff-add" : line.kind === "del" ? "diff-del" : "diff-ctx";
  const sign = line.kind === "add" ? "+" : line.kind === "del" ? "−" : " ";
  // Жёлоб показывает ту сторону, к которой строка относится (§6.3).
  const number = line.kind === "del" ? line.oldLine : line.newLine;

  return (
    <div className={mark ? `diff-line ${kind} diff-hit diff-hit-${mark}` : `diff-line ${kind}`}>
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
