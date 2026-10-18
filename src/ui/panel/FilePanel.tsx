/**
 * File panel: the file whole, as it lies now, at full height.
 *
 * Opened by clicking the file name in an action row, whether the file was created, changed or
 * only read. There is no diff here: what exactly the call changed is shown by its expanded row
 * in the feed, while the panel answers a different question: what is in the file in the end.
 *
 * This is a viewer, not an editor: line numbers and highlighting, no folding of blocks and no
 * cursor. The file name sits on the panel's tab; under the header is a line with the path and
 * three actions on the file: search by text, copy, and expanding the panel to the whole window.
 *
 * The text comes from the core (`fileViewLoaded`) as segments with hunks; the lines of the new
 * side are taken from them, and removed lines are dropped.
 */

import {
  ArrowsInSimple,
  ArrowsOutSimple,
  CaretDown,
  CaretUp,
  Check,
  Code,
  Copy,
  Eye,
  MagnifyingGlass,
  X,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";

import type { DiffLine, FileView } from "../../core/client";
import type { Feed } from "../../core/feed";
import { t } from "../../i18n";
import { Diff } from "../Diff";
import { plainPath } from "../format";
import { MarkdownDoc } from "../MarkdownDoc";
import { useCopy } from "../useCopy";
import { Panel } from "./Panel";

export function FilePanel({
  path,
  feed,
  onRead,
}: {
  path: string;
  feed: Feed;
  /** Asks the core for the whole file. */
  onRead: (path: string) => void;
}) {
  // The request goes out when the panel shows another file and when the agent writes the file
  // again: `writes` grows with every write, and the view has to follow it. The callback is read
  // through a ref so that a parent re-render does not send the request again: otherwise the
  // core's answer and the re-render would feed each other.
  const readRef = useRef(onRead);
  useEffect(() => {
    readRef.current = onRead;
  });
  useEffect(() => {
    readRef.current(path);
  }, [path, feed.writes]);

  // `undefined`: the core has not answered for this file yet; `null`: nothing to show, the file
  // is missing, binary, or lies outside the project.
  const viewed = feed.fileView;
  const shown = viewed !== null && viewed.path === path ? viewed.view : undefined;
  const lines = useMemo(() => (shown ? currentLines(shown) : []), [shown]);
  // A deleted file arrives as a view of deleted lines only: the file is gone.
  const gone = shown === null || (shown !== undefined && lines.length === 0);
  const ready = shown !== undefined && shown !== null && !gone;

  const [full, setFull] = useState(false);
  // The rendered view exists only for Markdown; a search runs over the lines
  // of the code view, so opening the render closes it.
  const markdown = /\.(md|markdown|mdown|mkd)$/i.test(path);
  const [view, setView] = useState<"code" | "render">("code");
  const rendered = markdown && view === "render";
  const text = useMemo(() => lines.map((line) => line.text).join("\n"), [lines]);
  const { copied, copy } = useCopy();

  // `null`: search is closed. An open search with an empty string finds nothing.
  const [query, setQuery] = useState<string | null>(null);
  const [step, setStep] = useState(0);
  const found = useMemo(() => find(lines, query ?? ""), [lines, query]);
  // A different query or a different file means a different list of hits; a step taken on the
  // old list means nothing in the new one.
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
        if (ready && !rendered && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
          event.preventDefault();
          setQuery((now) => now ?? "");
        }
      }}
    >
      <div className="file-bar">
        <span className="file-path">{plainPath(path)}</span>
        {markdown && (
          <div className="file-view" role="group">
            {(["code", "render"] as const).map((kind) => {
              const label = t(kind === "code" ? "file.view_code" : "file.view_render");
              return (
                <button
                  key={kind}
                  className="file-view-btn"
                  aria-pressed={view === kind}
                  title={label}
                  onClick={() => {
                    setView(kind);
                    if (kind === "render") setQuery(null);
                  }}
                >
                  {kind === "code" ? <Code size={14} /> : <Eye size={14} />}
                  {label}
                </button>
              );
            })}
          </div>
        )}
        <button
          className="top-icon"
          aria-label={t("file.search")}
          title={t("file.search")}
          aria-pressed={query !== null}
          disabled={!ready || rendered}
          onClick={() => setQuery((now) => (now === null ? "" : null))}
        >
          <MagnifyingGlass size={15} />
        </button>
        <button
          className="top-icon"
          aria-label={copyLabel}
          title={copyLabel}
          disabled={!ready}
          onClick={() => copy(text)}
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

      {query !== null && !rendered && (
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
        {ready && rendered && (
          <div className="md-scroll">
            <MarkdownDoc text={text} />
            {shown.truncated && <div className="md-note">{t("file.truncated")}</div>}
          </div>
        )}
        {ready && !rendered && (
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
