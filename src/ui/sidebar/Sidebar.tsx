/**
 * Left sidebar: the navigation column of the window.
 *
 * Not a drawer §1.1. A drawer is opened for one look and gets in the way once
 * it stays; this list is looked at all day, so it lives in the frame as a
 * column of its own and the feed shrinks beside it instead of going inert
 * underneath. Processes and the file panel stay drawers — they cover the feed
 * because they are read against it, not next to it.
 *
 * Projects are the rows, chats are what folds out of them, and both are many:
 * several projects open at once, several chats open inside one. What the
 * window is showing is one of those rows; the rest keep working.
 *
 * The new-chat rows lead to the home screen §9.1 rather than opening a session
 * of their own: a session is born with the first message, and a button that
 * left an empty row in the list would say otherwise.
 *
 * One search field over every project, not one per group: «which conversation
 * was that in» is the question being asked, and it is asked before the project
 * is remembered.
 */

import { FolderPlus, Gear, Plus } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { t } from "../../i18n";
import { ProjectGroup } from "./ProjectGroup";
import type { Chat } from "../chats";
import type { SessionId } from "../../core/client";

/** Search over the journals is linear in their size (README); the debounce
    keeps it to one pass per pause rather than one per keystroke. */
const SEARCH_DEBOUNCE_MS = 200;

export function Sidebar({
  projects,
  chats,
  activeId,
  current,
  reload,
  providersOpen,
  onNewChat,
  onOpenSession,
  onCloseChat,
  onAddProject,
  onForgetProject,
  onOpenProviders,
}: {
  /** Projects the window knows about (§9.1), freshest first. */
  projects: string[];
  /** Every chat open in the window, in any project. */
  chats: Chat[];
  activeId: SessionId | null;
  /** Where the row at the top starts a chat: the project in view, or none. */
  current: string | null;
  /** Changes when the store could have — reopen, rename, finished turn. */
  reload: string;
  providersOpen: boolean;
  onNewChat: (project: string) => void;
  onOpenSession: (project: string, id: SessionId) => void;
  onCloseChat: (id: SessionId) => void;
  onAddProject: () => void;
  onForgetProject: (project: string) => void;
  onOpenProviders: () => void;
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
    <nav className="side" aria-label={t("top.sessions")}>
      <div className="side-head">
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

      <div className="side-foot">
        <button className="side-row" onClick={onAddProject}>
          <span className="side-icon" aria-hidden="true">
            <FolderPlus size={15} />
          </span>
          {t("side.add_project")}
        </button>

        {/* Trigger of drawer §1.1: the attribute is what makes a second click
            close the panel instead of reopening it (§1.1). */}
        <button
          className="side-row"
          data-panel-trigger
          aria-pressed={providersOpen}
          onClick={onOpenProviders}
        >
          <span className="side-icon" aria-hidden="true">
            <Gear size={15} />
          </span>
          {t("prov.title")}
        </button>
      </div>
    </nav>
  );
}
