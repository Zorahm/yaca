import { flatRows, type ActionRow, type Feed } from "./feed";

/** The core names the process in the launch result, including timeout handoffs. */
export function backgroundProcessId(row: ActionRow): number | null {
  if (row.tool !== "Bash") return null;
  const match = /^(?:Running in background with id |Did not complete within \d+ s — continues running in background with id )(\d+)\b/m.exec(row.output);
  return match ? Number(match[1]) : null;
}

export function inProcessesPanel(row: ActionRow): boolean {
  if (row.tool === "BashWait" && row.state === "running") return true;
  return row.state === "done" && backgroundProcessId(row) !== null;
}

/** Derive names from the recorded launch, so identical commands keep their own captions. */
export function processTitles(feed: Feed): Map<number, string> {
  const titles = new Map<number, string>();
  for (const block of feed.blocks) {
    if (block.kind !== "actions") continue;
    for (const row of flatRows(block.rows)) {
      const id = backgroundProcessId(row);
      if (id !== null && row.description?.trim()) titles.set(id, row.description);
    }
  }
  return titles;
}
