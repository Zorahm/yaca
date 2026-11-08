import { CaretDown, CaretRight, CaretUp } from "@phosphor-icons/react";
import { useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { capitalize, t, tp } from "../i18n";
import { ActionRowView } from "./ActionRow";
import { actionNotices } from "./editNotices";
import { actionFailed, activityPath, fileDirectories, pathKey, type FileActivity as Activity } from "./fileActivity";
import { baseName } from "./format";
import { FileLspBadge } from "./LspBadge";
import { RowSign, Spinner } from "./RowSign";

export function FileActivity({ group, onOpenFile }: {
  group: Activity;
  onOpenFile: (path: string, line?: number) => void;
}) {
  const [expanded, setExpanded] = useState<boolean>();
  const filesId = useId();
  const [diffs, setDiffs] = useState<{ all: boolean; overrides: Map<string, boolean> }>(() => ({
    all: false, overrides: new Map(),
  }));
  const repeats = new Map<string, number>();
  const rows = group.steps.map(({ row }) => {
    const path = pathKey(activityPath(row)!);
    const attempt = (repeats.get(path) ?? 0) + 1;
    repeats.set(path, attempt);
    const notices = actionNotices(row);
    const error = notices.some((notice) => notice.errors > 0);
    return { row, path, attempt, notices, error, failed: actionFailed(row) };
  });
  const paths = [...repeats.keys()];
  const { prefix, compact, count } = fileDirectories(paths);
  const directory = compact || tp("file_group.directories", count);
  const name = baseName(paths[0]);
  const sameName = paths.every((path) => baseName(path) === name);
  const current = rows.find(({ row }) => row.state === "running");
  const completed = rows.filter(({ row }) => row.state !== "running").length;
  const successful = new Set(rows.filter(({ row }) => row.state === "done").map(({ path }) => path)).size;
  const failed = rows.filter((item) => item.failed).length;
  const open = expanded ?? failed > 0;
  const repeatedEdit = paths.length === 1 && group.action === "modified";
  const title = repeatedEdit
    ? tp("file_group.repeated_edit", rows.length, { name })
    : capitalize(tp(`group.${group.action}.with`, successful));
  const showName = sameName && !repeatedEdit;
  const counts = rows.reduce((sum, { row }) => ({
    added: sum.added + (row.fact?.added ?? 0),
    removed: sum.removed + (row.fact?.removed ?? 0),
  }), { added: 0, removed: 0 });
  const isOpen = (item: typeof rows[number]) =>
    diffs.overrides.get(item.row.callId) ?? (item.error || (diffs.all && item.row.diff !== null));
  const hasDiffs = rows.some(({ row }) => row.diff);
  const allDiffsOpen = hasDiffs && rows.every((item) => !item.row.diff || isOpen(item));
  const ordered = open ? [...rows].sort((left, right) => Number(right.error) - Number(left.error)) : [];

  return (
    <div className="file-activity" data-open={open || undefined}>
      <button className="file-activity-head" aria-expanded={open} aria-controls={open ? filesId : undefined}
        onClick={() => setExpanded(!open)}>
        {current ? <Spinner /> : <RowSign row={rows.find((item) => !item.failed)?.row ?? rows[0].row} />}
        <span className="file-activity-title">
          {title}{showName && <> · <span className="row-name">{name}</span></>}
        </span>
        <span className="file-activity-stats">
          {(counts.added > 0 || counts.removed > 0) && <span className="counts counts-pill">
            <span className="count-add">+{counts.added}</span>
            <span className="count-del">−{counts.removed}</span>
          </span>}
          {failed > 0 && <span className="group-failed">{tp("group.failed", failed)}</span>}
          <FileLspBadge files={rows.map(({ notices }) => notices)} />
        </span>
        <span className="caret">{open ? <CaretDown size={12} /> : <CaretRight size={12} />}</span>
      </button>
      <div className="file-activity-meta">
        <span className="file-activity-path" title={directory}>{directory}</span>
        {open && hasDiffs && <button className="btn btn-row file-activity-toggle"
          onClick={() => setDiffs({ all: !allDiffsOpen, overrides: allDiffsOpen
            ? new Map(rows.filter(({ row }) => row.diff).map(({ row }) => [row.callId, false]))
            : new Map() })}>
          {allDiffsOpen ? <CaretUp size={12} aria-hidden="true" /> : <CaretDown size={12} aria-hidden="true" />}
          {t(allDiffsOpen ? "file_group.hide_diffs" : "file_group.show_diffs")}
        </button>}
        {current && <span className="file-activity-current" title={current.path}>
          <span aria-label={t("file_group.progress", { completed, total: rows.length })}>{completed}/{rows.length}</span>
          <span>{current.path.slice(prefix.length)}</span>
        </span>}
      </div>
      {open && <div id={filesId} className="file-activity-files">
        {ordered.map((item) => <ActionRowView key={item.row.callId} row={item.row} onOpenFile={onOpenFile}
          fileDetail={{
            label: item.path.slice(prefix.length),
            directoryPrefix: prefix,
            sharedName: sameName,
            attempt: (repeats.get(item.path) ?? 0) > 1 ? item.attempt : undefined,
            open: isOpen(item),
            onToggle: () => setDiffs((previous) => ({
              ...previous,
              overrides: new Map(previous.overrides).set(item.row.callId, !isOpen(item)),
            })),
          }} />)}
      </div>}
    </div>
  );
}

/** Absorbed calls keep their slots so the space they occupied can close smoothly. */
export function FileActivitySlot({ variant, live, children }: {
  variant: "single" | "group" | "folded";
  live: boolean;
  children?: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const previous = useRef({ variant, height: 0 });
  useLayoutEffect(() => {
    const slot = ref.current!;
    const body = slot.firstElementChild!;
    const height = body.getBoundingClientRect().height;
    const before = previous.current;
    let animation: Animation | undefined;
    if (live && before.variant !== variant && before.height > 0 && before.height !== height &&
        !matchMedia("(prefers-reduced-motion: reduce)").matches) {
      animation = slot.animate([
        { height: `${before.height}px`, overflow: "clip" },
        { height: `${height}px`, overflow: "clip" },
      ], { duration: 180, easing: "ease-out" });
    }
    previous.current = { variant, height };
    if (variant === "folded") return () => animation?.cancel();
    const observer = new ResizeObserver(() => {
      previous.current.height = body.getBoundingClientRect().height;
    });
    observer.observe(body);
    return () => {
      observer.disconnect();
      animation?.cancel();
    };
  }, [variant, live]);

  return <div ref={ref} className="file-activity-slot" data-folded={variant === "folded" || undefined}>
    <div className="file-activity-slot-body">{children}</div>
  </div>;
}
