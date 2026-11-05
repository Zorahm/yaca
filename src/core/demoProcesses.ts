import type { AgentEvent } from "./bindings/AgentEvent";
import type { Command } from "./bindings/Command";
import type { ProcessSummary } from "./bindings/ProcessSummary";

const now = Date.now();
const seeded: ProcessSummary[] = [
  { id: 1, command: "npm run dev -- --host 127.0.0.1", cwd: ".", state: { type: "running" }, startedAt: now - 164000, finishedAt: null, lines: 6, dropped: 0 },
  { id: 2, command: "npm test -- segments", cwd: ".", state: { type: "exited", code: 0 }, startedAt: now - 42000, finishedAt: now - 38000, lines: 6, dropped: 0 },
  { id: 3, command: "npm run build -- --mode production", cwd: ".", state: { type: "exited", code: 1 }, startedAt: now - 32000, finishedAt: now - 25000, lines: 4, dropped: 0 },
];
const output: Record<number, string> = {
  1: "> yaca@1.0.5-beta dev\n> vite --host 127.0.0.1\n\n  VITE v7.3.6 ready in 848 ms\n\n  Local: http://127.0.0.1:1420/\n",
  2: " RUN  v4.1.11\n\n ✓ src/segments.test.ts (12 tests)\n\n Test Files  1 passed (1)\n      Tests  12 passed (12)\n",
  3: "> tsc && vite build --mode production\n\nsrc/segments.ts:14:22 - error TS2365\nOperator '+' cannot be applied to these types.\n",
};
const sessions = new Map<string, ProcessSummary[]>();

export const demoBackgroundEvents: AgentEvent[] = [
  ...seeded.flatMap((process): AgentEvent[] => [
    { type: "toolCallStarted", turnId: "t1", callId: `bg${process.id}`, tool: "Bash", description: process.id === 1 ? "Started preview server" : process.id === 2 ? "Ran parser tests" : null, target: process.command, input: { command: process.command, background: true }, parent: null },
    { type: "toolCallProgress", callId: `bg${process.id}`, chunk: `Running in background with id ${process.id}. Read its output with BashOutput.\n` },
    { type: "toolCallFinished", callId: `bg${process.id}`, meta: { lines: 1, items: null, truncated: false, durationMs: 12 } },
  ]),
  { type: "processesUpdated", processes: seeded },
  { type: "toolCallStarted", turnId: "t1", callId: "bgwait", tool: "BashWait", description: "Waiting for the preview server", target: null, input: { id: 1, until: "ready" }, parent: null },
];

/** The stand answers the same output subscription and stop commands as the core. */
export function demoProcessCommand(session: string, command: Command): AgentEvent[] | null {
  if (command.type !== "watchProcess" && command.type !== "stopProcess") return null;
  const processes = sessions.get(session) ?? structuredClone(seeded);
  sessions.set(session, processes);
  if (command.type === "watchProcess") {
    if (command.id === null) return [];
    const process = processes.find((process) => process.id === command.id);
    return process ? [{ type: "processOutputReset", id: process.id, text: output[process.id] ?? "", dropped: 0 }] : [];
  }
  const process = processes.find((process) => process.id === command.id);
  if (process?.state.type === "running") {
    process.state = { type: "stopped" };
    process.finishedAt = Date.now();
  }
  return [{ type: "processesUpdated", processes: structuredClone(processes) }];
}
