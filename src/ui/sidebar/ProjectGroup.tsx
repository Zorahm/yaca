/**
 * One project in the sidebar: the header row and, when open, its sessions.
 *
 * The header is the project's own line rather than a heading over the list:
 * it starts a chat, it folds the list away, and it drops the project from the
 * window. Folding is what makes several projects readable at once — a dozen
 * projects with twenty sessions each is a wall, and a wall hides exactly what
 * the sidebar is for.
 *
 * A folded project still says how many of its chats are open. State that
 * disappears when you fold it away is state the interface is lying about (§0).
 */

import { CaretDown, CaretRight, Plus, X } from "@phosphor-icons/react";

import { t } from "../../i18n";
import { baseName } from "../format";
import { SessionList } from "./SessionList";
import type { Chat } from "../chats";
import type { SessionId } from "../../core/client";

export function ProjectGroup({
  project,
  chats,
  activeId,
  query,
  reload,
  expanded,
  onToggle,
  onNewChat,
  onForget,
  onOpen,
  onClose,
}: {
  project: string;
  /** Chats of this project open in the window right now. */
  chats: Chat[];
  activeId: SessionId | null;
  query: string;
  reload: string;
  expanded: boolean;
  onToggle: () => void;
  onNewChat: () => void;
  onForget: () => void;
  onOpen: (id: SessionId) => void;
  onClose: (id: SessionId) => void;
}) {
  return (
    <section className="side-group">
      <div className="side-project">
        <button
          className="side-row side-project-main"
          onClick={onToggle}
          aria-expanded={expanded}
          title={project}
        >
          <span className="side-icon" aria-hidden="true">
            {expanded ? <CaretDown size={12} /> : <CaretRight size={12} />}
          </span>
          <span className="side-project-name">{baseName(project)}</span>
          {chats.length > 0 && (
            <span className="side-project-live" title={t("side.open")}>
              {chats.length}
            </span>
          )}
        </button>

        <button
          className="side-project-act"
          aria-label={t("side.new_here")}
          title={t("side.new_here")}
          onClick={onNewChat}
        >
          <Plus size={13} />
        </button>

        {/* Dropping a project with chats in it would leave them running with
            no row to come back to. The button says so instead of doing it.

            Size 13 like every other action glyph in the sidebar, the plus
            beside it included: a cross set one pixel smaller because a
            rotated bar "looks bigger" reads as a character next to an icon
            rather than as its pair. */}
        <button
          className="side-project-act side-project-drop"
          aria-label={t("side.forget")}
          title={chats.length > 0 ? t("side.forget_busy") : t("side.forget")}
          disabled={chats.length > 0}
          onClick={onForget}
        >
          <X size={13} />
        </button>
      </div>

      {expanded && (
        <SessionList
          project={project}
          open={chats}
          activeId={activeId}
          query={query}
          reload={reload}
          onOpen={onOpen}
          onClose={onClose}
        />
      )}
    </section>
  );
}
