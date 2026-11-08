import type { ActionRow } from "../core/feed";
import type { WorkStep } from "../core/work";
import { plainPath } from "./format";

type FileAction = "read" | "modified" | "created" | "deleted";
type FileStep = Extract<WorkStep, { kind: "action" }>;

export type FileActivity = {
  action: FileAction;
  steps: FileStep[];
};

const fileTools: Partial<Record<string, FileAction>> = {
  Read: "read",
  Edit: "modified",
  NotebookEdit: "modified",
  Write: "created",
  Delete: "deleted",
};

export function activityPath(row: ActionRow): string | null {
  return row.fact?.path ?? (Object.prototype.hasOwnProperty.call(fileTools, row.tool) ? row.target : null);
}

export function pathKey(path: string): string {
  return plainPath(path).replace(/\\/g, "/").replace(/^(?:\.\/)+/, "");
}

/** Anchor each group to its first call, even when reads and edits alternate. */
export function groupFileSteps(steps: WorkStep[]): Map<string, FileActivity | null> {
  const byAction = new Map<FileAction, FileStep[]>();
  for (const step of steps) {
    if (step.kind !== "action" || !activityPath(step.row)) continue;
    const action = step.row.fact?.kind ?? fileTools[step.row.tool];
    if (!action || action === "moved") continue;
    const peers = byAction.get(action);
    if (peers) peers.push(step);
    else byAction.set(action, [step]);
  }
  const groups = new Map<string, FileActivity | null>();
  for (const [action, peers] of byAction) {
    if (peers.length < (action === "read" ? 2 : 3)) continue;
    groups.set(peers[0].key, { action, steps: peers });
    for (const step of peers.slice(1)) groups.set(step.key, null);
  }
  return groups;
}

/** Each distinct path gets its shortest unique suffix. Repeated calls share it. */
export function uniqueFileNames(paths: string[]): Map<string, string> {
  const keys = [...new Set(paths.map(pathKey))];
  const counts = new Map<string, number>();
  for (const path of keys) {
    let start = path.lastIndexOf("/");
    while (start >= 0) {
      const suffix = path.slice(start + 1);
      counts.set(suffix, (counts.get(suffix) ?? 0) + 1);
      start = start === 0 ? -1 : path.lastIndexOf("/", start - 1);
    }
    counts.set(path, (counts.get(path) ?? 0) + 1);
  }
  return new Map(keys.map((path) => {
    let start = path.lastIndexOf("/");
    while (start >= 0 && counts.get(path.slice(start + 1)) !== 1) {
      start = start === 0 ? -1 : path.lastIndexOf("/", start - 1);
    }
    return [path, path.slice(start + 1)];
  }));
}

export function fileDirectories(paths: string[]) {
  const directories = [...new Set(paths.map((path) => path.slice(0, path.lastIndexOf("/") + 1)))];
  let prefix = directories[0] ?? "";
  for (const directory of directories) {
    while (prefix && !directory.startsWith(prefix)) {
      prefix = prefix.slice(0, prefix.slice(0, -1).lastIndexOf("/") + 1);
    }
  }
  const branches = directories.map((directory) => directory.slice(prefix.length, -1) || ".");
  const hasCommonDirectory = Boolean(prefix && !/^(?:\/+|[A-Za-z]:\/|src\/)$/.test(prefix));
  const short = branches.slice(0, 4).join(", ");
  const more = branches.length > 4 ? `, …+${branches.length - 4}` : "";
  const compact = directories.length === 1 ? prefix : hasCommonDirectory ? `${prefix}{${short}${more}}/` : "";
  return { prefix: hasCommonDirectory || directories.length === 1 ? prefix : "", compact, count: directories.length };
}

export function actionFailed(row: ActionRow): boolean {
  return row.state === "failed" || row.state === "rejected" || row.state === "denied" || row.state === "stopped";
}
