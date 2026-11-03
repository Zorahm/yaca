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
 *
 * In a narrow window (`isNarrow`) there is no room for a column, and the same
 * list becomes a drawer over the feed: shut until the top bar's button opens
 * it, shut again by the scrim, a swipe, Back, or by picking where to go.
 */

import { ArrowCircleUp, Gear, Plus, X } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";

import { t } from "../../i18n";
import { YacaLockup } from "../Logo";
import { SIDE, type Bounds } from "../resize";
import { Segments } from "../settings/Row";
import { Splitter } from "../Splitter";
import { ProjectGroup } from "./ProjectGroup";
import { SessionList } from "./SessionList";
import { PROFILES, PROFILE_KEY } from "../ProfileMenu";
import { useSwipeShut } from "./useSwipeShut";
import type { Chat } from "../chats";
import type { Profile, ReleaseInfo, SessionId } from "../../core/client";

/** Search over the journals is linear in their size (README); the debounce
    keeps it to one pass per pause rather than one per keystroke. */
const SEARCH_DEBOUNCE_MS = 200;

export function Sidebar({
  projects,
  profile,
  onProfile,
  chatRoot,
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
  drawer = null,
}: {
  /** Projects the window knows about, freshest first. */
  projects: string[];
  profile: Profile;
  onProfile: (profile: Profile) => void;
  chatRoot: string | null;
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
  /**
   * A drawer over the feed instead of a column beside it; `null` in a window
   * wide enough for the column. Mounted shut too, for the same reason a
   * collapsed column is: it has to come back as it was left.
   */
  drawer?: { open: boolean; onShut: () => void } | null;
}) {
  const [typed, setTyped] = useState("");
  const self = useRef<HTMLElement>(null);
  const scrim = useRef<HTMLDivElement>(null);
  const swipe = useSwipeShut(self, scrim, () => drawer?.onShut());
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

  const column = (
    <nav
      ref={self}
      className={drawer === null ? "side" : "side side-drawer"}
      aria-label={t("top.sessions")}
      style={drawer !== null ? undefined : collapsed ? { display: "none" } : { width }}
      data-open={drawer?.open}
      // Shut, it is off the screen, and neither a tab nor a screen reader
      // should walk into it there.
      inert={drawer !== null && !drawer.open}
      onKeyDown={(event) => {
        if (drawer !== null && event.key === "Escape") drawer.onShut();
      }}
      {...(drawer === null ? {} : swipe)}
    >
      {drawer === null && (
        <Splitter
          edge="right"
          width={width}
          bounds={bounds}
          initial={SIDE.initial}
          label={t("side.resize")}
          onResize={onResize}
        />
      )}
      <div className="side-head">
        <div className="side-top">
          <YacaLockup height={24} />
          <Segments
            label={t("profile.title")}
            value={profile}
            options={PROFILES.map((item) => ({ value: item, label: t(PROFILE_KEY[item]) }))}
            onChange={onProfile}
          />
        </div>

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
        {profile === "chat" && chatRoot !== null ? (
          <SessionList project={chatRoot} open={chats.filter((chat) => chat.project === chatRoot)}
            activeId={activeId} query={query} reload={reload}
            onOpen={(id) => onOpenSession(chatRoot, id)} onClose={onCloseChat} />
        ) : profile === "code" && <>
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
        </>}
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

  if (drawer === null) return column;
  return (
    <>
      {/* A tap outside the drawer shuts it; the keyboard has Esc for that,
          so the scrim is no control of its own. */}
      <div
        ref={scrim}
        className="side-scrim"
        data-open={drawer.open}
        aria-hidden="true"
        onClick={drawer.onShut}
      />
      {column}
    </>
  );
}
