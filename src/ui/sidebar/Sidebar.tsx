/**
 * Left sidebar: the navigation column of the window.
 *
 * This list is looked at all day, so it lives in the frame as a column of its
 * own and the feed shrinks beside it. The panel on the right is the same kind
 * of column; both are dragged to the width a person wants.
 *
 * Projects are the rows, chats are what folds out of them, and both are many:
 * several projects open at once, several chats open inside one. What the
 * window is showing is one of those rows; the rest keep working.
 *
 * The new-chat rows lead to the home screen rather than opening a session
 * of their own: a session is born with the first message, and a button that
 * left an empty row in the list would say otherwise.
 *
 * One search field over every project, not one per group: «which conversation
 * was that in» is the question being asked, and it is asked before the project
 * is remembered.
 */

import { ArrowCircleUp, Gear, Plus, X } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { t } from "../../i18n";
import { ownTitleBar } from "../../core/frame";
import { YacaMark } from "../Logo";
import { SIDE, type Bounds } from "../resize";
import { Splitter } from "../Splitter";
import { ProjectGroup } from "./ProjectGroup";
import type { Chat } from "../chats";
import type { ReleaseInfo, SessionId } from "../../core/client";

/** Search over the journals is linear in their size (README); the debounce
    keeps it to one pass per pause rather than one per keystroke. */
const SEARCH_DEBOUNCE_MS = 200;

export function Sidebar({
  projects,
  chats,
  activeId,
  current,
  reload,
  onNewChat,
  onOpenSession,
  onCloseChat,
  onForgetProject,
  onOpenSettings,
  update = null,
  onInstallUpdate,
  onUpdateDetails,
  onDismissUpdate,
  width,
  bounds,
  onResize,
  collapsed = false,
}: {
  /** Projects the window knows about, freshest first. */
  projects: string[];
  /** Every chat open in the window, in any project. */
  chats: Chat[];
  activeId: SessionId | null;
  /** Where the row at the top starts a chat: the project in view, or none. */
  current: string | null;
  /** Changes when the store could have — reopen, rename, finished turn. */
  reload: string;
  onNewChat: (project: string) => void;
  onOpenSession: (project: string, id: SessionId) => void;
  onCloseChat: (id: SessionId) => void;
  onForgetProject: (project: string) => void;
  /** The footer button opens the settings window, not a panel. */
  onOpenSettings: () => void;
  /** A newer release than the running one, once the check has found it. */
  update?: ReleaseInfo | null;
  onInstallUpdate: () => void;
  /** Leads to About, where the changelog is. */
  onUpdateDetails: () => void;
  /** Hides the card until the application is restarted. */
  onDismissUpdate: () => void;
  /** The column's width and how far the handle on its edge may take it. */
  width: number;
  bounds: Bounds;
  onResize: (width: number) => void;
  /**
   * Hidden, not unmounted. Folded away and brought back, the column has to
   * be what it was: which projects were open, what was typed in the search,
   * the lists already read. Unmounting it dropped all of that, and the
   * sessions «disappeared» until each project was opened again.
   */
  collapsed?: boolean;
}) {
  const [typed, setTyped] = useState("");
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<Set<string>>(
    () => new Set(current === null ? [] : [current]),
  );

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(typed), typed ? SEARCH_DEBOUNCE_MS : 0);
    return () => window.clearTimeout(timer);
  }, [typed]);

  // The project being worked in unfolds itself. Folding it back is allowed —
  // this only reacts to the project changing, not to every render.
  useEffect(() => {
    if (current === null) return;
    setExpanded((now) => (now.has(current) ? now : new Set(now).add(current)));
  }, [current]);

  function toggle(project: string) {
    setExpanded((now) => {
      const next = new Set(now);
      if (!next.delete(project)) next.add(project);
      return next;
    });
  }

  return (
    <nav
      className="side"
      aria-label={t("top.sessions")}
      style={collapsed ? { display: "none" } : { width }}
    >
      <Splitter
        edge="right"
        width={width}
        bounds={bounds}
        initial={SIDE.initial}
        label={t("side.resize")}
        onResize={onResize}
      />
      <div className="side-head">
        {/* Where the window draws its own title bar the mark and the name
            are in it, a few pixels above; twice in one corner is once too
            many. */}
        {!ownTitleBar && (
          <div className="side-brand">
            <YacaMark size={22} />
            Yaca
          </div>
        )}

        <button
          className="side-row side-new"
          disabled={current === null}
          onClick={() => current !== null && onNewChat(current)}
        >
          <span className="side-icon" aria-hidden="true">
            <Plus size={15} />
          </span>
          {t("side.new")}
        </button>

        <input
          className="side-search"
          type="search"
          value={typed}
          placeholder={t("sess.search_placeholder")}
          aria-label={t("sess.search_placeholder")}
          onChange={(e) => setTyped(e.target.value)}
        />
      </div>

      <div className="side-scroll">
        {projects.length > 0 && <div className="side-label">{t("side.projects")}</div>}
        {projects.length === 0 ? (
          <div className="side-empty">{t("side.no_projects")}</div>
        ) : (
          projects.map((project) => (
            <ProjectGroup
              key={project}
              project={project}
              chats={chats.filter((chat) => chat.project === project)}
              activeId={activeId}
              query={query}
              reload={reload}
              // Searching unfolds everything: a match hidden in a folded
              // project is a search that answered «nothing» and meant «not
              // here».
              expanded={expanded.has(project) || query !== ""}
              onToggle={() => toggle(project)}
              onNewChat={() => onNewChat(project)}
              onForget={() => onForgetProject(project)}
              onOpen={(id) => onOpenSession(project, id)}
              onClose={onCloseChat}
            />
          ))
        )}
      </div>

      {update !== null && (
        <div className="side-update" role="status">
          <span className="side-update-title">
            <ArrowCircleUp size={15} aria-hidden="true" />
            {t("about.update_available", { version: update.version })}
          </span>
          <button
            className="side-update-close"
            onClick={onDismissUpdate}
            aria-label={t("notice.dismiss")}
          >
            <X size={14} />
          </button>
          <span className="side-update-actions">
            <button className="btn btn-accent btn-small" onClick={onInstallUpdate}>
              {t("side.update_do")}
            </button>
            <button className="btn btn-small" onClick={onUpdateDetails}>
              {t("side.update_more")}
            </button>
          </span>
        </div>
      )}

      <div className="side-foot">
        {/* A window, not a panel: settings outlive the conversation, and they are
            opened without closing the feed. */}
        <button className="side-row side-row-large" onClick={onOpenSettings}>
          <span className="side-icon" aria-hidden="true">
            <Gear size={18} />
          </span>
          {t("set.title")}
        </button>
      </div>
    </nav>
  );
}
