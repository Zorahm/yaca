import type { AgentEvent } from "./bindings/AgentEvent";
import type { CheckpointChange } from "./bindings/CheckpointChange";
import type { CheckpointSummary } from "./bindings/CheckpointSummary";
import type { Command } from "./bindings/Command";

const now = Date.now();
export const demoCheckpoints: CheckpointSummary[] = [
  { id: "ckpt_1", at: now - 24 * 60000, reason: { type: "message" }, tracked: 148 },
  { id: "ckpt_2", at: now - 12 * 60000, reason: { type: "command", command: "npm run dev -- --host 127.0.0.1" }, tracked: 149 },
  { id: "ckpt_3", at: now - 6 * 60000, reason: { type: "command", command: "npm test -- segments" }, tracked: 150 },
];
const comparisons: Record<string, CheckpointChange[]> = {
  ckpt_1: [
    { path: "src/segments.ts", kind: "modified", added: 1, removed: 2 },
    { path: "src/legacy/zone.ts", kind: "created", added: 8, removed: 0 },
    { path: "scripts/check-date.ts", kind: "deleted", added: 0, removed: 18 },
  ],
  ckpt_2: [{ path: "src/segments.ts", kind: "modified", added: 1, removed: 1 }],
  ckpt_3: [],
};
type DemoState = { points: CheckpointSummary[]; restored: string | null; preview: string | null; tracked: number; undo: Record<string, CheckpointChange[]> };
const sessions = new Map<string, DemoState>();

/** Recorded comparisons let the stand exercise preview, confirmation, and undo. */
export function demoCheckpointCommand(session: string, command: Command): AgentEvent[] | null {
  if (command.type !== "previewCheckpoint" && command.type !== "restoreCheckpoint") return null;
  const state: DemoState = sessions.get(session) ?? { points: structuredClone(demoCheckpoints), restored: null, preview: null, tracked: 150, undo: {} };
  sessions.set(session, state);
  const point = state.points.find((point) => point.id === command.id);
  if (!point) return [];
  const files = state.restored === point.id ? [] : state.undo[point.id] ?? comparisons[point.id] ?? [];
  if (command.type === "previewCheckpoint") {
    state.preview = point.id;
    return [{ type: "checkpointPreviewed", id: point.id, files: structuredClone(files) }];
  }
  if (state.preview !== point.id || files.length === 0) return [];
  const undo: CheckpointSummary = {
    id: `ckpt_undo_${state.points.length}`, at: Date.now(), reason: { type: "rollback" }, tracked: state.tracked,
  };
  state.undo[undo.id] = files.map((file) => ({
    ...file, kind: file.kind === "created" ? "deleted" : file.kind === "deleted" ? "created" : "modified",
    added: file.removed, removed: file.added,
  }));
  state.points.push(undo);
  state.restored = point.id;
  state.tracked = point.tracked;
  state.preview = null;
  return [
    { type: "checkpointRestored", id: point.id, at: point.at, files: files.length },
    { type: "checkpointCreated", checkpoint: undo },
    { type: "checkpointsUpdated", checkpoints: structuredClone(state.points) },
  ];
}
