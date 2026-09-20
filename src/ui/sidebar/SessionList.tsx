/**
 * The sessions of one project (§9): days, rows, per-row actions.
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

import { DotsThree, X } from "@phosphor-icons/react";
import { DropdownMenu } from "radix-ui";
import { useEffect, useState } from "react";

import {
  chooseSavePath,
  sessions,
  type SearchHit,
  type SessionId,
  type SessionMeta,
} from "../../core/client";
import { t } from "../../i18n";
import { money, timeOf } from "../format";
import { useFlash } from "../useFlash";
import type { Chat } from "../chats";

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
      {groupByDay(rows).map(([day, dayRows]) => (
        <section key={day}>
          <div className="side-day">{day}</div>
          {dayRows.map(({ session, excerpt }) => {
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
                confirming={confirming === session.id}
                onOpen={() => onOpen(session.id)}
                onClose={() => onClose(session.id)}
                onRenameStart={() => setRenaming(session.id)}
                onRenameEnd={() => setRenaming(null)}
                onRename={(title) => {
                  setRenaming(null);
                  void act(() => sessions.rename(project, session.id, title));
                }}
                onDuplicate={() => void act(() => sessions.duplicate(project, session.id))}
                onExport={async () => {
                  const path = await chooseSavePath(
                    fileName(session.title),
                    t("sess.export"),
                    "md",
                  );
                  if (path === null) return false;
                  await sessions.exportTo(project, session.id, path);
                  return true;
                }}
                onError={(e) => setError(String(e))}
                onDeleteStart={() => setConfirming(session.id)}
                onDeleteCancel={() => setConfirming(null)}
                onDelete={() => {
                  setConfirming(null);
                  void act(() => sessions.remove(project, session.id));
                }}
              />
            );
          })}
        </section>
      ))}
    </>
  );
}

function SessionRow({
  row,
  live,
  renaming,
  confirming,
  onOpen,
  onClose,
  onRenameStart,
  onRenameEnd,
  onRename,
  onDuplicate,
  onExport,
  onError,
  onDeleteStart,
  onDeleteCancel,
  onDelete,
}: {
  row: Row;
  /** `undefined` — the session is recorded but not open in the window. */
  live: Live | undefined;
  renaming: boolean;
  confirming: boolean;
  onOpen: () => void;
  onClose: () => void;
  onRenameStart: () => void;
  onRenameEnd: () => void;
  onRename: (title: string) => void;
  onDuplicate: () => void;
  /** Asks where to save and saves.  — the human changed their mind. */
  onExport: () => Promise<boolean>;
  onError: (error: unknown) => void;
  onDeleteStart: () => void;
  onDeleteCancel: () => void;
  onDelete: () => void;
}) {
  const { session, excerpt } = row;
  const { shown: saved, flash } = useFlash();
  const [menu, setMenu] = useState(false);
  const title = session.title || t("top.untitled");

  return (
    <div
      className={rowClass(live)}
      // Правый клик по строке открывает то же меню, что и `⋮`. Один список на
      // два входа: второй компонент Radix означал бы два набора пунктов,
      // которые однажды разъедутся.
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu(true);
      }}
    >
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
          // The bar and the fill say «this one» to the eye; `aria-current`
          // says it to everything else (§14).
          aria-current={live?.active ? "true" : undefined}
          title={title}
        >
          {/* The dot is a slot, filled only for a chat that is open: rows
              would step left and right without it. What the dot says is what
              the chat is doing — working, or waiting for an answer (§0). */}
          <span
            className={liveClass(live)}
            aria-label={live && t(live.waiting ? "side.waiting" : "side.open")}
          />
          <span className="sess-text">
            <span className="sess-title">{title}</span>
            {excerpt && <span className="sess-excerpt">{excerpt}</span>}
            <span className="sess-meta">
              {timeOf(session.updatedAt)} · ${money(session.costUsd)}
            </span>
          </span>
        </button>
      )}

      {/* Managing a session is four actions, and four buttons standing on a
          272 px row would be the row — they used to lie over the title behind
          a gradient. One `⋮` instead, holding the same list §3 already names
          for the session menu in the top bar; the row's own right click opens
          it too. The trigger comes out on hover and on focus — `opacity`, not
          `display`, so the keyboard still walks in (§14). */}
      <DropdownMenu.Root
        open={menu}
        onOpenChange={(next) => {
          setMenu(next);
          // Закрытое меню забывает и начатое удаление: пункт, оставшийся
          // подтверждением до следующего открытия, удалил бы с одного нажатия.
          if (!next) onDeleteCancel();
        }}
      >
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
          <DropdownMenu.Content className="menu menu-pop" align="end" sideOffset={4}>
            <DropdownMenu.Item className="menu-item" onSelect={onRenameStart}>
              {t("sess.rename")}
            </DropdownMenu.Item>
            <DropdownMenu.Item className="menu-item" onSelect={onDuplicate}>
              {t("sess.duplicate")}
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="menu-item"
              onSelect={(e) => {
                // Меню остаётся открытым: «Экспортировано» нужно где-то
                // показать, а диалог сохранения и так забрал фокус.
                e.preventDefault();
                void onExport()
                  .then((done) => done && flash())
                  .catch(onError);
              }}
            >
              {saved ? t("sess.exported") : t("sess.export")}
            </DropdownMenu.Item>
            <div className="menu-sep" />
            <DropdownMenu.Item
              className="menu-item menu-item-danger"
              onSelect={(e) => {
                if (confirming) {
                  onDelete();
                  return;
                }
                // Первый выбор не удаляет: меню остаётся открытым, а пункт
                // становится вопросом. Необратимое требует второго нажатия.
                e.preventDefault();
                onDeleteStart();
              }}
            >
              {confirming ? t("sess.delete_confirm") : t("sess.delete")}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      {/* Closing is not one of them: it stops the session and leaves the
          journal, so it stands out of the hover group and stays visible while
          the chat is open — the way the cross of a tab does. */}
      {live && (
        <button
          className="sess-close"
          aria-label={t("side.close_chat")}
          title={t("side.close_chat")}
          onClick={onClose}
        >
          <X size={13} />
        </button>
      )}
    </div>
  );
}

function rowClass(live: Live | undefined): string {
  if (!live) return "sess";
  return live.active ? "sess sess-open sess-current" : "sess sess-open";
}

function liveClass(live: Live | undefined): string {
  if (!live) return "sess-live";
  if (live.waiting) return "sess-live sess-live-waiting";
  if (live.busy) return "sess-live sess-live-busy pulse";
  return "sess-live sess-live-open";
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

/**
 * Grouped by the day of last activity (§9). Order is the store's: the list
 * comes back sorted by time, and the groups do not re-cut it.
 */
function groupByDay(rows: Row[]): Array<[string, Row[]]> {
  const groups = new Map<string, Row[]>();
  for (const row of rows) {
    const day = dayLabel(row.session.updatedAt);
    const bucket = groups.get(day);
    if (bucket) bucket.push(row);
    else groups.set(day, [row]);
  }
  return [...groups];
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
