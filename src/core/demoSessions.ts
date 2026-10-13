/**
 * Recorded sessions for the window without a shell.
 *
 * The sidebar is a list of sessions, and a stand whose list is always empty
 * shows none of it: no rows, no menu, no open chat among closed ones. These
 * are a handful of sessions kept in memory, and they answer the edits of §9
 * the way the store would — a row renamed here is renamed in the next list,
 * a row deleted is gone. A stand that took the click and kept the list as it
 * was would be lying about its own state (§0).
 *
 * Used only when Tauri is absent, like the rest of `core/demo`.
 */

import type { AgentEvent } from "./bindings/AgentEvent";
import type { SearchHit } from "./bindings/SearchHit";
import type { SessionId } from "./bindings/SessionId";
import type { SessionMeta } from "./bindings/SessionMeta";
import { demoState } from "./demo";

const HOUR = 3_600_000;
const DAY = 24 * HOUR;

type Seed = {
  title: string;
  /** How long ago the session was last touched. */
  ago: number;
  messages: number;
  costUsd: number;
  files: string[];
  titleCustom?: boolean;
};

/**
 * Ages are counted back from now, so the stand always has rows of today,
 * of yesterday and of last month. The titles differ in length on purpose:
 * one that does not fit, one that is a single word, one that is empty and
 * falls back to «untitled».
 */
const SEEDS: Seed[] = [
  {
    title: "Найди все файлы с русскими комментариями и составь список",
    ago: 20 * 60_000,
    messages: 6,
    costUsd: 0.42,
    files: [],
  },
  {
    title: "Продолжай.",
    ago: 3 * HOUR,
    messages: 14,
    costUsd: 1.87,
    files: ["src/ui/sidebar/SessionList.tsx", "src/ui/sidebar/sidebar.css"],
  },
  {
    title: "Привет!",
    ago: DAY + 2 * HOUR,
    messages: 2,
    costUsd: 0.03,
    files: [],
  },
  {
    title: "Кнопка копирования у ответа модели",
    ago: DAY + 5 * HOUR,
    messages: 4,
    costUsd: 0.31,
    files: ["src/ui/Blocks.tsx", "src/ui/Feed.tsx", "src/ui/feed.css"],
    titleCustom: true,
  },
  {
    title: "",
    ago: 4 * DAY,
    messages: 1,
    costUsd: 0.01,
    files: [],
  },
  {
    title: "Спланируй новую функцию: черновики сообщений в композере",
    ago: 24 * DAY,
    messages: 9,
    costUsd: 0.94,
    files: ["src/ui/Composer.tsx"],
  },
  {
    title: "Отредактируй .md файл в папке проекта",
    ago: 42 * DAY,
    messages: 3,
    costUsd: 0.12,
    files: ["README.md"],
  },
  {
    title: "Почему падает cargo test на Windows",
    ago: 43 * DAY,
    messages: 11,
    costUsd: 2.4,
    files: ["crates/yaca-core/src/testutil.rs"],
  },
];

function seeded(): SessionMeta[] {
  const now = Date.now();
  return SEEDS.map((seed, index) => ({
    id: `ses_rec${index + 1}`,
    title: seed.title,
    titleCustom: seed.titleCustom ?? false,
    createdAt: now - seed.ago - HOUR,
    updatedAt: now - seed.ago,
    provider: demoState.provider,
    model: demoState.model,
    messages: seed.messages,
    costUsd: seed.costUsd,
    tokens: { input: 0, output: 0, cacheWrite: 0, cacheRead: 0 },
    files: seed.files,
    forkedFrom: null,
  }));
}

/** The stand's project has sessions; one added by hand starts with none. */
const store = new Map<string, SessionMeta[]>([[demoState.projectRoot, seeded()]]);
let nextCopy = 1;

function of(project: string): SessionMeta[] {
  return store.get(project) ?? [];
}

function find(project: string, id: SessionId): SessionMeta | undefined {
  return of(project).find((session) => session.id === id);
}

export const demoSessions = {
  /** Freshest first, as the store returns them. */
  list(project: string): SessionMeta[] {
    return [...of(project)].sort((a, b) => b.updatedAt - a.updatedAt);
  },

  /** By title and by the paths a session changed: there are no messages here. */
  search(project: string, query: string): SearchHit[] {
    const needle = query.toLowerCase();
    const hits: SearchHit[] = [];
    for (const session of demoSessions.list(project)) {
      const path = session.files.find((file) => file.toLowerCase().includes(needle));
      if (session.title.toLowerCase().includes(needle)) {
        hits.push({ session, matches: [{ kind: "title", excerpt: session.title }] });
      } else if (path !== undefined) {
        hits.push({ session, matches: [{ kind: "path", excerpt: path }] });
      }
    }
    return hits;
  },

  rename(project: string, id: SessionId, title: string): void {
    const session = find(project, id);
    if (session) Object.assign(session, { title, titleCustom: true });
  },

  remove(project: string, id: SessionId): void {
    store.set(
      project,
      of(project).filter((session) => session.id !== id),
    );
  },

  duplicate(project: string, id: SessionId): SessionMeta {
    const source = find(project, id);
    if (!source) throw new Error(`no such session: ${id}`);
    const now = Date.now();
    const copy: SessionMeta = {
      ...source,
      id: `ses_copy${nextCopy}`,
      createdAt: now,
      updatedAt: now,
      forkedFrom: source.id,
    };
    nextCopy += 1;
    store.set(project, [...of(project), copy]);
    return copy;
  },

  /** Whether `id` is one of the recorded sessions, in any project. */
  meta(id: SessionId): SessionMeta | null {
    for (const list of store.values()) {
      const session = list.find((one) => one.id === id);
      if (session) return session;
    }
    return null;
  },

  /**
   * What an opened session draws: the message it is named after and a line
   * of answer. Enough for the feed not to be empty under an open row.
   */
  feed(project: string, id: SessionId): AgentEvent[] {
    const session = find(project, id);
    if (!session || session.title === "") return [];
    return [
      { type: "userMessage", turnId: "t1", text: session.title, attachments: [] },
      {
        type: "textDelta",
        turnId: "t1",
        blockId: "b1",
        text: "Это записанная сессия стенда: журнала за ней нет, только строка в списке.",
      },
    ];
  },
};
