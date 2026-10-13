/**
 * Панель файла: файл целиком, как он лежит сейчас, на всю высоту.
 *
 * Открывается кликом по имени файла в строке действия — созданного,
 * изменённого или только прочитанного. Диффа здесь нет: что именно поменял
 * вызов, показывает его раскрытая строка в ленте, а панель отвечает на другой
 * вопрос — что в файле в итоге.
 *
 * Это просмотр, а не редактор: номера строк и подсветка, без сворачивания
 * блоков и без курсора. Имя файла стоит на вкладке панели; под шапкой — строка
 * с путём и тремя действиями над файлом: поиск по тексту, копирование и
 * раскрытие панели на всё окно.
 *
 * Текст приходит от ядра (`fileViewLoaded`) в виде сегментов с ханками; из них
 * берутся строки новой стороны, удалённые отбрасываются.
 */

import {
  ArrowsInSimple,
  ArrowsOutSimple,
  CaretDown,
  CaretUp,
  Check,
  Copy,
  MagnifyingGlass,
  X,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";

import type { DiffLine, FileView } from "../../core/client";
import type { Feed } from "../../core/feed";
import { t } from "../../i18n";
import { Diff } from "../Diff";
import { plainPath } from "../format";
import { useCopy } from "../useCopy";
import { Panel } from "./Panel";

export function FilePanel({
  path,
  feed,
  onRead,
}: {
  path: string;
  feed: Feed;
  /** Просит ядро отдать файл целиком. */
  onRead: (path: string) => void;
}) {
  // Запрос уходит, когда панель показала другой файл и когда агент записал
  // файл ещё раз: `writes` растёт с каждой записью, и вид должен идти за ней.
  // Колбэк читается через ref, чтобы перерисовка родителя не
  // слала запрос заново — иначе ответ ядра и перерисовка кормили бы друг друга.
  const readRef = useRef(onRead);
  useEffect(() => {
    readRef.current = onRead;
  });
  useEffect(() => {
    readRef.current(path);
  }, [path, feed.writes]);

  // `undefined` — ядро ещё не ответило за этот файл; `null` — показать
  // нечего: файла нет, он двоичный или лежит вне проекта.
  const viewed = feed.fileView;
  const shown = viewed !== null && viewed.path === path ? viewed.view : undefined;
  const lines = useMemo(() => (shown ? currentLines(shown) : []), [shown]);
  // Удалённый файл приходит видом из одних удалённых строк: файла больше нет.
  const gone = shown === null || (shown !== undefined && lines.length === 0);
  const ready = shown !== undefined && shown !== null && !gone;

  const [full, setFull] = useState(false);
  const { copied, copy } = useCopy();

  // `null` — поиск закрыт. Открытый с пустой строкой ничего не находит.
  const [query, setQuery] = useState<string | null>(null);
  const [step, setStep] = useState(0);
  const found = useMemo(() => find(lines, query ?? ""), [lines, query]);
  // Другой запрос или другой файл — другой список находок; шаг по старому
  // списку в нём ничего не значит.
  useEffect(() => setStep(0), [found]);
  const hits = useMemo(() => new Set(found), [found]);
  const move = (by: number) =>
    found.length > 0 && setStep((now) => (now + by + found.length) % found.length);

  const copyLabel = t(copied ? "feed.copied" : "file.copy");
  const fullLabel = t(full ? "file.shrink" : "file.expand");

  return (
    <Panel
      full={full}
      // `Esc` closes the search first and the panel only after it.
      onEscape={(event) => {
        if (query === null) return;
        event.preventDefault();
        setQuery(null);
      }}
      onKeyDown={(event) => {
        if (ready && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
          event.preventDefault();
          setQuery((now) => now ?? "");
        }
      }}
    >
      <div className="file-bar">
        <span className="file-path">{plainPath(path)}</span>
        <button
          className="top-icon"
          aria-label={t("file.search")}
          title={t("file.search")}
          aria-pressed={query !== null}
          disabled={!ready}
          onClick={() => setQuery((now) => (now === null ? "" : null))}
        >
          <MagnifyingGlass size={15} />
        </button>
        <button
          className="top-icon"
          aria-label={copyLabel}
          title={copyLabel}
          disabled={!ready}
          onClick={() => copy(lines.map((line) => line.text).join("\n"))}
        >
          {copied ? <Check size={15} weight="bold" /> : <Copy size={15} />}
        </button>
        <button
          className="top-icon"
          aria-label={fullLabel}
          title={fullLabel}
          onClick={() => setFull(!full)}
        >
          {full ? <ArrowsInSimple size={15} /> : <ArrowsOutSimple size={15} />}
        </button>
      </div>

      {query !== null && (
        <div className="file-search">
          <input
            className="file-search-input"
            autoFocus
            value={query}
            placeholder={t("file.search_placeholder")}
            aria-label={t("file.search")}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") move(event.shiftKey ? -1 : 1);
            }}
          />
          <span className="file-search-count">
            {query === ""
              ? ""
              : found.length === 0
                ? t("file.search_none")
                : t("file.search_count", { at: step + 1, of: found.length })}
          </span>
          <button
            className="top-icon"
            aria-label={t("file.search_prev")}
            title={t("file.search_prev")}
            disabled={found.length === 0}
            onClick={() => move(-1)}
          >
            <CaretUp size={14} />
          </button>
          <button
            className="top-icon"
            aria-label={t("file.search_next")}
            title={t("file.search_next")}
            disabled={found.length === 0}
            onClick={() => move(1)}
          >
            <CaretDown size={14} />
          </button>
          <button
            className="top-icon"
            aria-label={t("file.search_close")}
            title={t("file.search_close")}
            onClick={() => setQuery(null)}
          >
            <X size={14} />
          </button>
        </div>
      )}

      <div className="panel-file">
        {shown === undefined && <div className="panel-empty">{t("file.loading")}</div>}
        {gone && <div className="panel-empty">{t("file.unavailable")}</div>}
        {ready && (
          <Diff
            body={{
              lines,
              truncated: shown.truncated,
              hits,
              current: found.length > 0 ? found[step] : null,
            }}
            path={path}
          />
        )}
      </div>
    </Panel>
  );
}

/**
 * The file as it is now: every line of the new side, none of them marked.
 * Word-level spans go too — they point at what changed, and nothing is shown
 * as changed here.
 */
function currentLines(view: FileView): DiffLine[] {
  return view.segments
    .flatMap((segment) => (segment.kind === "hunk" ? segment.hunk.lines : segment.lines))
    .filter((line) => line.kind !== "del")
    .map((line) => ({ ...line, kind: "context" as const, spans: [] }));
}

/** Indexes of the lines that hold the query, case ignored. */
function find(lines: DiffLine[], query: string): number[] {
  const needle = query.toLowerCase();
  if (needle === "") return [];
  const out: number[] = [];
  for (const [at, line] of lines.entries()) {
    if (line.text.toLowerCase().includes(needle)) out.push(at);
  }
  return out;
}
