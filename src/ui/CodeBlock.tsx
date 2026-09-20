/**
 * Блок кода и блок вывода команды (§5.2, §5.5.1).
 *
 * Одна и та же вёрстка: §5.2 прямо говорит, что блоки кода в тексте агента
 * оформляются как блок вывода, с кнопкой копирования.
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
 * Окна, как у диффа (§12), здесь не нужно: это один `pre` с одним текстовым
 * узлом, а не десять тысяч элементов.
 */

import { Check, Copy } from "@phosphor-icons/react";

import { t } from "../i18n";
import { useCopy } from "./useCopy";

export function CodeBlock({
  text,
  error = false,
}: {
  text: string;
  error?: boolean;
}) {
  const { copied, copy } = useCopy();
  const label = t(copied ? "feed.copied" : "feed.copy");

  return (
    <div className="code-wrap">
      <pre className={error ? "output output-error" : "output"}>{text}</pre>
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
