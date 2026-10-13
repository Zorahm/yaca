/**
 * The frame of the right panel: a column of the window beside the feed.
 *
 * It is not a dialog. The feed stays live next to it, nothing is dimmed, and
 * focus is not trapped: the panel is read against the conversation, and a
 * person answers the agent with it open. `Esc` closes it only from inside,
 * because everywhere else `Esc` already means «stop the agent».
 *
 * One panel is open at a time and the window owns which: the header is the
 * strip of tabs that switches between them, and the same context carries the
 * column's width, since both are the window's state and not the business of
 * the four panels drawn inside.
 *
 * When the window is too narrow to keep the feed readable beside it
 * (`bounds` is `null`), the panel lies over the feed instead and cannot be
 * dragged.
 */

import { FileText, X } from "@phosphor-icons/react";
import {
  createContext,
  useContext,
  type KeyboardEvent,
  type ReactNode,
} from "react";

import { t, type Key } from "../../i18n";
import { baseName } from "../format";
import type { Bounds } from "../resize";
import { Splitter } from "../Splitter";

export type PanelKind = "checkpoints" | "processes" | "mcp" | "agents" | "file";

export type PanelFrame = {
  current: PanelKind;
  /**
   * The file behind the `file` tab. The tab is listed only while there is
   * one: a tab with nothing behind it would be a button that does nothing.
   */
  filePath: string | null;
  /** The conversation has sent subagents: only then is there an `agents` tab. */
  hasAgents: boolean;
  onSwitch: (kind: PanelKind) => void;
  onClose: () => void;
  width: number;
  /** `null` — no room beside the feed: the panel lies over it. */
  bounds: Bounds | null;
  /** Where a double click on the handle returns the column. */
  initialWidth: number;
  onResize: (width: number) => void;
};

export const PanelFrameContext = createContext<PanelFrame | null>(null);

const TABS: { kind: Exclude<PanelKind, "file">; label: Key }[] = [
  { kind: "checkpoints", label: "top.checkpoints" },
  { kind: "processes", label: "top.processes" },
  { kind: "mcp", label: "panel.tab_mcp" },
  { kind: "agents", label: "panel.tab_agents" },
];

export function Panel({
  full = false,
  onEscape,
  onKeyDown,
  children,
}: {
  /** Over the whole feed column instead of beside it: a file opened wide. */
  full?: boolean;
  /** `Esc` before it closes the panel: prevent the event to keep it open. */
  onEscape?: (event: KeyboardEvent) => void;
  onKeyDown?: (event: KeyboardEvent) => void;
  children: ReactNode;
}) {
  const frame = useContext(PanelFrameContext);
  // A panel is only ever drawn by the window, which provides the frame.
  if (frame === null) return null;

  const fileName = frame.filePath === null ? null : baseName(frame.filePath);
  const name =
    frame.current === "file"
      ? (fileName ?? "")
      : t(TABS.find((tab) => tab.kind === frame.current)?.label ?? "top.processes");

  const tab = (kind: PanelKind, label: ReactNode, title?: string) => (
    <button
      key={kind}
      role="tab"
      aria-selected={kind === frame.current}
      className={
        (kind === frame.current ? "panel-tab panel-tab-on" : "panel-tab") +
        (kind === "file" ? " panel-tab-file" : "")
      }
      title={title}
      onClick={() => kind !== frame.current && frame.onSwitch(kind)}
    >
      {label}
    </button>
  );

  return (
    <aside
      className={
        full
          ? "panel-dock panel-dock-full"
          : frame.bounds === null
            ? "panel-dock panel-dock-over"
            : "panel-dock"
      }
      style={full ? undefined : { width: frame.width }}
      aria-label={name}
      // Focusable by a click, so that keys pressed over the panel reach it.
      tabIndex={-1}
      onKeyDown={(event) => {
        onKeyDown?.(event);
        if (event.key !== "Escape") return;
        // The window's own `Esc` stops the agent; this one is the panel's.
        event.stopPropagation();
        onEscape?.(event);
        if (!event.defaultPrevented) frame.onClose();
      }}
    >
      {!full && frame.bounds !== null && (
        <Splitter
          edge="left"
          width={frame.width}
          bounds={frame.bounds}
          initial={frame.initialWidth}
          label={t("panel.resize")}
          onResize={frame.onResize}
        />
      )}
      <div className="panel">
        <header className="panel-head">
          <div className="panel-tabs" role="tablist">
            {TABS.filter(
              // The open tab stays listed even with nothing behind it: another
              // chat was switched to, and the panel says so itself.
              ({ kind }) => kind !== "agents" || frame.hasAgents || frame.current === "agents",
            ).map(({ kind, label }) => tab(kind, t(label)))}
            {fileName !== null &&
              frame.filePath !== null &&
              tab(
                "file",
                <>
                  <FileText size={14} />
                  <span className="panel-tab-name">{fileName}</span>
                </>,
                frame.filePath,
              )}
          </div>
          <button
            className="top-icon"
            aria-label={t("common.close")}
            title={t("common.close")}
            onClick={frame.onClose}
          >
            <X size={14} />
          </button>
        </header>
        {children}
      </div>
    </aside>
  );
}
