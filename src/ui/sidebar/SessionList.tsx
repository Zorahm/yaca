/**
 * The sessions of one project (§9): rows and their menu.
 *
 * One flat list, freshest first, as the store returns it. Headings by day cut
 * a dozen rows into five groups of two and cost a line each; when a chat was
 * last touched is in the tooltip of its row.
 *
 * Reads the store, which is per project and exists whether or not anyone is
 * talking in it. What is open in the window is laid over that list rather than
 * kept beside it: an open chat is one of these rows with a dot, and clicking
 * it brings that chat up instead of starting a second live session on the same
 * journal.
 *
 * The list stays mounted, so it cannot rely on being reopened to be fresh:
 * `reload` changes whenever the store could have, and reading it again is
 * cheaper than a list that lies.
 *
 * Export goes to a file: the shell owns the save dialog and the core owns the
 * writing, so the row only asks where. The Markdown itself never passes
 * through the window.
 *
 * «Duplicate up to a point» from §9 is the whole session here: picking the
 * point needs the list of turns, which this list does not have. The button
 * says what it does.
 */

import { DotsThree } from "@phosphor-icons/react";
import { ContextMenu, DropdownMenu } from "radix-ui";
import { useEffect, useState } from "react";

import {
  chooseSavePath,
  sessions,
  type SearchHit,
  type SessionId,
  type SessionMeta,
} from "../../core/client";
import { t, type Key } from "../../i18n";
import { money, timeOf } from "../format";
import type { Chat } from "../chats";
import { SessionMenuItems, type SessionCommands } from "./SessionMenu";

type Row = { session: SessionMeta; excerpt: string | null };

/** What the window knows about a row that is open: enough to mark it. */
type Live = { busy: boolean; waiting: boolean; active: boolean };

export function SessionList({
  project,
  open,
  activeId,
  query,
  reload,
  onOpen,
  onClose,
}: {
  project: string;
  /** Chats of this project open in the window right now. */
  open: Chat[];
  activeId: SessionId | null;
  /** Search from the sidebar: one field over every project (§9). */
  query: string;
  reload: string;
  onOpen: (id: SessionId) => void;
  onClose: (id: SessionId) => void;
}) {
  const [rows, setRows] = useState<Row[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<SessionId | null>(null);
  const [confirming, setConfirming] = useState<SessionId | null>(null);

  useEffect(() => {
    let live = true;
    void load(project, query).then(
      (next) => {
        if (live) {
          setRows(next);
          setError(null);
        }
      },
      (e) => {
        if (live) setError(String(e));
      },
    );
    return () => {
      live = false;
    };
  }, [project, query, reload]);

  async function act(action: () => Promise<unknown>) {
    try {
      await action();
      setRows(await load(project, query));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  if (error) return <div className="side-error">{error}</div>;
  if (rows === null) return null;
  if (rows.length === 0) {
    // Under a query an empty group says «nothing here» by being empty; saying
    // it once per project would be five lines of the same word.
    return query ? null : <div className="side-empty">{t("sess.empty")}</div>;
  }

  return (
    <>
      {rows.map(({ session, excerpt }) => {
        const chat = open.find((one) => one.id === session.id);
        return (
          <SessionRow
            key={session.id}
            row={{ session, excerpt }}
            live={
              chat && {
                busy: chat.feed.state?.busy === true,
                waiting: chat.feed.request !== null,
                active: chat.id === activeId,
              }
            }
            renaming={renaming === session.id}
            onOpen={() => onOpen(session.id)}
            onRenameEnd={() => setRenaming(null)}
            onRename={(title) => {
              setRenaming(null);
              void act(() => sessions.rename(project, session.id, title));
            }}
            onMenuClose={() => setConfirming(null)}
            commands={{
              onClose: chat ? () => onClose(session.id) : null,
              onRename: () => setRenaming(session.id),
              onDuplicate: () => void act(() => sessions.duplicate(project, session.id)),
              onExport: async () => {
                const path = await chooseSavePath(
                  fileName(session.title),
                  t("sess.export"),
                  "md",
                );
                if (path === null) return false;
                await sessions.exportTo(project, session.id, path);
                return true;
              },
              onError: (e) => setError(String(e)),
              confirming: confirming === session.id,
              onDeleteStart: () => setConfirming(session.id),
              onDelete: () => {
                setConfirming(null);
                void act(() => sessions.remove(project, session.id));
              },
            }}
          />
        );
      })}
    </>
  );
}

function SessionRow({
  row,
  live,
  renaming,
  onOpen,
  onRenameEnd,
  onRename,
  onMenuClose,
  commands,
}: {
  row: Row;
  /** `undefined` — the session is recorded but not open in the window. */
  live: Live | undefined;
  renaming: boolean;
  onOpen: () => void;
  onRenameEnd: () => void;
  onRename: (title: string) => void;
  /** Either menu of the row has closed. */
  onMenuClose: () => void;
  commands: SessionCommands;
}) {
  const { session, excerpt } = row;
  const title = session.title || t("top.untitled");
  const mark = markOf(live);

  // A closed menu forgets a delete that was only asked about: an item left as
  // the question until the next opening would delete on one press.
  const onOpenChange = (open: boolean) => {
    if (!open) onMenuClose();
  };

  return (
    <ContextMenu.Root onOpenChange={onOpenChange}>
      <ContextMenu.Trigger asChild>
        <div className={rowClass(live)}>
          {renaming ? (
            <input
              className="sess-rename"
              defaultValue={title}
              autoFocus
              aria-label={t("sess.rename")}
              onBlur={(e) => {
                if (e.target.value.trim()) onRename(e.target.value.trim());
                else onRenameEnd();
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  const value = e.currentTarget.value.trim();
                  if (value) onRename(value);
                  else onRenameEnd();
                }
                if (e.key === "Escape") onRenameEnd();
              }}
            />
          ) : (
            <button
              className="sess-main"
              onClick={onOpen}
              // The fill says «this one» to the eye; `aria-current` says it to
              // everything else (§14).
              aria-current={live?.active ? "true" : undefined}
              // One line per chat: when and how much live in the tooltip.
              title={`${title}
${whenOf(session.updatedAt)} · $${money(session.costUsd)}`}
            >
              <span className="sess-text">
                <span className="sess-title">{title}</span>
                {excerpt && <span className="sess-excerpt">{excerpt}</span>}
              </span>
            </button>
          )}

          {/* A chat that is doing something says so at the end of its row:
              working, or waiting for an answer (§0). A chat that is idle has
              nothing to say, and a mark on every row said nothing either. */}
          {mark && (
            <span className={mark.className} role="img" aria-label={t(mark.label)} />
          )}

          {/* Everything a row can do is in its menu, closing the chat
              included: a cross on every open row was a second column of
              controls beside the titles. The trigger comes out on hover and
              on focus — `opacity`, not `display`, so the keyboard still walks
              in (§14). */}
          <DropdownMenu.Root onOpenChange={onOpenChange}>
            <DropdownMenu.Trigger asChild>
              <button
                className="sess-more"
                aria-label={t("top.session_menu")}
                title={t("top.session_menu")}
              >
                <DotsThree size={16} weight="bold" />
              </button>
            </DropdownMenu.Trigger>
            <DropdownMenu.Portal>
              <DropdownMenu.Content className="pop" align="end" sideOffset={4}>
                <SessionMenuItems Item={DropdownMenu.Item} commands={commands} />
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        </div>
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="pop">
          <SessionMenuItems Item={ContextMenu.Item} commands={commands} />
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

function rowClass(live: Live | undefined): string {
  return live?.active ? "sess sess-current" : "sess";
}

function markOf(live: Live | undefined): { className: string; label: Key } | null {
  if (live?.waiting) return { className: "sess-live sess-live-waiting", label: "side.waiting" };
  if (live?.busy) return { className: "sess-live sess-live-busy pulse", label: "side.working" };
  return null;
}

async function load(project: string, query: string): Promise<Row[]> {
  if (query.trim()) {
    const hits: SearchHit[] = await sessions.search(project, query.trim());
    return hits.map((hit) => ({
      session: hit.session,
      // A hit in a message is the strongest reason to show the row; a hit in a
      // path or a title is still worth showing as the reason it is here.
      excerpt:
        hit.matches.find((m) => m.kind === "message")?.excerpt ??
        hit.matches[0]?.excerpt ??
        null,
    }));
  }
  const list: SessionMeta[] = await sessions.list(project);
  return list.map((session) => ({ session, excerpt: null }));
}

/** When a chat was last touched, for the tooltip: the day, then the time. */
function whenOf(ms: number): string {
  return `${dayLabel(ms)}, ${timeOf(ms)}`;
}

function dayLabel(ms: number): string {
  const date = new Date(ms);
  const today = new Date();
  const sameDay = (a: Date, b: Date) =>
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  // Day boundaries are local: a session touched at midnight today is not
  // «yesterday».
  if (sameDay(date, today)) return t("sess.today");
  if (sameDay(date, yesterday)) return t("sess.yesterday");
  return date.toLocaleDateString(undefined, { day: "numeric", month: "long" });
}

/**
 * Имя файла для экспорта: заголовок сессии без того, чего не бывает в именах.
 *
 * Диалог покажет его как предложенное — человек всё равно решает сам, поэтому
 * тут достаточно не подсунуть ему заведомо непринимаемое имя.
 */
function fileName(title: string): string {
  const clean = title.replace(/[\\/:*?"<>|]/g, "-").trim();
  return `${clean || t("top.untitled")}.md`;
}
