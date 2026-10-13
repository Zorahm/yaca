/**
 * Блок кода и блок вывода команды (§5.2, §5.5.1).
 *
 * Одна и та же вёрстка: §5.2 прямо говорит, что блоки кода в тексте агента
 * оформляются как блок вывода, с кнопкой копирования. Различаются они только
 * подсветкой: у блока кода есть язык из ограды ``` — его грамматика
 * загружается лениво, как у диффа (§6.3); у вывода языка нет, и остаётся он
 * простым текстом — вывод команды исходником не был.
 *
 * Кнопка живёт в углу самого блока и выходит по наведению — тем же приёмом,
 * которым §6.3 показывает кнопки на заголовке ханка. Отдельной кнопкой под
 * блоком она читалась как действие ленты, а не как действие над этим кодом, и
 * занимала строку в разговоре, где каждая строка — реплика.
 *
 * Вывод целиком, а не хвост: раскрытие, за которым нужен второй клик, — это
 * не раскрытие (§16.34). Длинный вывод не делает ленту простынёй, потому что
 * блок ограничен по высоте и скроллится сам (§5.5.1).
 *
 * Окна, как у диффа (§12), здесь не нужно: это один `pre`, а не десять тысяч
 * элементов.
 */

import { Check, Copy } from "@phosphor-icons/react";
import { Fragment, useMemo } from "react";
import type { ReactNode } from "react";

import { t } from "../i18n";
import { runsOf } from "./codeText";
import { plainLine, type Piece } from "./highlight";
import { fenceOf } from "./languages";
import { useHighlight } from "./useHighlight";
import { useCopy } from "./useCopy";

export function CodeBlock({
  text,
  info,
  error = false,
  plain = false,
}: {
  text: string;
  /** Язык из ограды — «ts», «rust»… У блока вывода ограды нет. */
  info?: string;
  error?: boolean;
  /** No well and no frame: the text sits on the page (a command's output). */
  plain?: boolean;
}) {
  const { copied, copy } = useCopy();
  const label = t(copied ? "feed.copied" : "feed.copy");
  const highlight = useHighlight(fenceOf(info ?? ""));

  // Строки режутся один раз на текст и подсветчик: поток гонит блок дельтами,
  // и пересборка при каждом кусочке дешевле, чем держать дерево узлов. Мемо
  // держится на стабильном подсветчике из useHighlight: дельта текста
  // пересобирает, случайный рендер — нет. `shown` берёт уже нарезанный
  // массив, второй split был бы просто местом для расхождения.
  const split = useMemo(() => text.split("\n"), [text]);
  const lines = useMemo(
    () => (highlight === null ? split.map(plainLine) : split.map((line) => highlight(line))),
    [split, highlight],
  );

  return (
    <div className="code-wrap">
      <pre
        className={["output", error && "output-error", plain && "output-plain"]
          .filter(Boolean)
          .join(" ")}
      >
        {shown(split, lines)}
      </pre>
      {info && <span className="code-lang">{info}</span>}
      {/* Иконка без подписи, поэтому имя обязательно (§14); `title` даёт то
          же мышью. */}
      <button
        className="block-act"
        aria-label={label}
        title={label}
        onClick={() => copy(text)}
      >
        {copied ? <Check size={13} weight="bold" /> : <Copy size={13} />}
      </button>
    </div>
  );
}

/**
 * Строки, склеенные настоящими переводами строки: `white-space` у `pre`
 * раскладывает их сам, и разрезать их пополам токеном невозможно. Строки
 * уже нарезаны при сборке `lines` — здесь только вторая половина пары.
 */
function shown(lines: readonly string[], pieces: readonly Piece[][]): ReactNode {
  const out: ReactNode[] = [];
  for (const [index, line] of lines.entries()) {
    if (index > 0) out.push("\n");
    // One fragment per line: the runs of a line are keyed from zero, and
    // spread into one list the keys of two lines would collide.
    out.push(<Fragment key={index}>{runsOf(line, pieces[index] ?? [])}</Fragment>);
  }
  return out;
}
