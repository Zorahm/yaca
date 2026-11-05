/**
 * Top bar.
 *
 * Holds navigation and the identification of the session. Background commands
 * have a direct link beside the turn status at the end of the feed.
 * There is no mark in the header — the square with a reflected `C` lives on the
 * home screen and in the app icon.
 *
 * At the edges are two buttons of one shape (`yaca-icons`): on the left, show or
 * hide the sidebar; on the right, the right panel. Each icon shows whether its
 * column is open; the right one carries a dot while background commands are
 * running. The button opens the panel on the tab it was closed on; the tabs
 * themselves are switched inside the panel. The arrows step the feed through
 * the human's messages.
 *
 * The globe left of the panel's button shows and hides the browser column,
 * in the order the two columns stand. Its teal dot means the model is using
 * a browser the human does not see right now.
 *
 * In a narrow window the left button opens the drawer the sidebar becomes
 * there, and wears the menu icon a phone expects in that corner.
 */

import { ArrowLeft, ArrowRight, Globe, List } from "@phosphor-icons/react";
import { useState } from "react";

import { t } from "../i18n";
import { baseName, plainPath } from "./format";
import { PanelLeft, PanelRight } from "./yaca-icons";

export function TopBar({
  title,
  projectRoot,
  projectEntry,
  folders,
  canGoBack,
  canGoForward,
  onBack,
  onForward,
  onRename,
  sidebarOpen,
  narrow = false,
  panelOpen,
  runningProcesses,
  onToggleSidebar,
  onTogglePanel,
  browser,
}: {
  title: string;
  projectRoot: string;
  projectEntry?: import("../core/client").ProjectEntry | null;
  folders?: string[];
  canGoBack: boolean;
  canGoForward: boolean;
  onBack: () => void;
  onForward: () => void;
  onRename: (title: string) => void;
  /** Whether the sidebar column on the left is shown, or the drawer open. */
  sidebarOpen: boolean;
  /** The sidebar is a drawer, not a column (`isNarrow`). */
  narrow?: boolean;
  /** Whether the right panel is open, on any tab. */
  panelOpen: boolean;
  /** The dot on the panel button — while there are live background commands. */
  runningProcesses: number;
  onToggleSidebar: () => void;
  onTogglePanel: () => void;
  /** `null` where this build has no browser. */
  browser: { open: boolean; busy: boolean; onToggle: () => void } | null;
}) {
  const [editing, setEditing] = useState<string | null>(null);

  return (
    <div className="top">
      {narrow ? (
        <button
          className="top-icon"
          aria-expanded={sidebarOpen}
          onClick={onToggleSidebar}
          aria-label={t("side.toggle")}
        >
          <List size={20} />
        </button>
      ) : (
        <button
          className="top-icon"
          aria-pressed={sidebarOpen}
          onClick={onToggleSidebar}
          aria-label={t("side.toggle")}
          title={t("side.toggle")}
        >
          <PanelLeft size={18} open={sidebarOpen} />
        </button>
      )}

      <button
        className="top-icon"
        onClick={onBack}
        disabled={!canGoBack}
        aria-label={t("top.back")}
        title={t("top.back")}
      >
        <ArrowLeft size={16} />
      </button>
      <button
        className="top-icon"
        onClick={onForward}
        disabled={!canGoForward}
        aria-label={t("top.forward")}
        title={t("top.forward")}
      >
        <ArrowRight size={16} />
      </button>

      {editing === null ? (
        <div
          className="top-title"
          title={t("top.rename_hint")}
          onDoubleClick={() => setEditing(title)}
        >
          {title || t("top.untitled")}
        </div>
      ) : (
        <input
          className="top-title"
          autoFocus
          value={editing}
          onChange={(e) => setEditing(e.target.value)}
          onBlur={() => setEditing(null)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              onRename(editing);
              setEditing(null);
            }
            if (e.key === "Escape") setEditing(null);
          }}
        />
      )}

      <span className="top-project" title={(folders ?? [projectRoot]).map(plainPath).join("\n")}>
        {projectEntry?.name ?? baseName(projectRoot)}
        {folders && folders.length > 1 && <span className="side-project-count"> · {folders.length}</span>}
      </span>

      <div className="top-gap" />

      {browser !== null && (
        <div className="top-icon-wrap">
          <button
            className="top-icon"
            aria-pressed={browser.open}
            onClick={browser.onToggle}
            aria-label={t("browser.toggle")}
            title={browser.busy && !browser.open ? t("browser.busy") : t("browser.toggle")}
          >
            <Globe size={18} />
          </button>
          {browser.busy && !browser.open && <span className="top-dot top-dot-accent" aria-hidden="true" />}
        </div>
      )}

      <div className="top-icon-wrap">
        <button
          className="top-icon"
          aria-pressed={panelOpen}
          onClick={onTogglePanel}
          aria-label={t("panel.toggle")}
          title={t("panel.toggle")}
        >
          <PanelRight size={18} open={panelOpen} />
        </button>
        {runningProcesses > 0 && <span className="top-dot" aria-hidden="true" />}
      </div>
    </div>
  );
}
