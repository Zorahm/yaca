/**
 * One project in the sidebar: the header row and, when open, its sessions.
 *
 * The header is the project's own line rather than a heading over the list:
 * it folds the list away and it starts a chat. Folding is what makes several
 * projects readable at once — a dozen projects with twenty sessions each is a
 * wall, and a wall hides exactly what the sidebar is for.
 *
 * Dropping the project from the window is in the row's right-click menu, not
 * on the row: it is done once in a project's life, and a cross that stood
 * beside the plus was one slip of the pointer away from it every day.
 */

import { CaretDown, CaretRight, FolderMinus, Plus } from "@phosphor-icons/react";
import { ContextMenu } from "radix-ui";

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
  // Dropping a project with chats in it would leave them running with no row
  // to come back to. The item is switched off and says why.
  const busy = chats.length > 0;

  return (
    <section className="side-group">
      <ContextMenu.Root>
        <ContextMenu.Trigger asChild>
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
            </button>

            <button
              className="side-project-act"
              aria-label={t("side.new_here")}
              title={t("side.new_here")}
              onClick={onNewChat}
            >
              <Plus size={13} />
            </button>
          </div>
        </ContextMenu.Trigger>
        <ContextMenu.Portal>
          <ContextMenu.Content className="pop">
            <ContextMenu.Item className="pop-item" onSelect={onNewChat}>
              <Plus size={14} />
              {t("side.new_here")}
            </ContextMenu.Item>
            <div className="pop-sep" />
            <ContextMenu.Item className="pop-item" disabled={busy} onSelect={onForget}>
              <FolderMinus size={14} />
              {t("side.forget")}
            </ContextMenu.Item>
            {busy && <div className="pop-note">{t("side.forget_busy")}</div>}
          </ContextMenu.Content>
        </ContextMenu.Portal>
      </ContextMenu.Root>

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
