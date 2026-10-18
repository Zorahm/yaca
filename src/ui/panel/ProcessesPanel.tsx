/**
 * Processes panel: background commands, their state, run time, and the output of the
 * selected row.
 *
 * The core sends the whole list in the `processesUpdated` event, so the panel is a function of
 * it and keeps no list of its own. Selecting a row sends `watchProcess`: the core answers
 * `processOutputReset` (the whole buffer) and then live chunks, and only for the selected row.
 *
 * The run time is counted by the interface: a core ticking an event every second would tell it
 * nothing new. While the panel is open and there are live processes, the time ticks; for
 * finished ones it is computed from `finishedAt`.
 */

import { Circle } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import type { ProcessId, ProcessState, ProcessSummary } from "../../core/client";
import type { Feed } from "../../core/feed";
import { t, tp } from "../../i18n";
import { durationOf } from "../format";
import { useCopy } from "../useCopy";
import { Panel } from "./Panel";

export function ProcessesPanel({
  feed,
  watched,
  onWatch,
  onStop,
}: {
  feed: Feed;
  /** The selected row. The selection itself lives in the core; the panel needs it here
      to restore the selection when the panel is reopened. */
  watched: ProcessId | null;
  onWatch: (id: ProcessId) => void;
  onStop: (id: ProcessId) => void;
}) {
  const processes = feed.processes;
  const selected = processes.find((p) => p.id === watched) ?? null;
  const running = processes.some((p) => p.state.type === "running");

  // Once a second while there are live ones: finished processes keep their time.
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => setTick((n) => n + 1), 1000);
    return () => window.clearInterval(timer);
  }, [running]);

  // Show exactly the buffer that arrived for the selected row.
  const output =
    watched !== null && feed.processOutput?.id === watched
      ? feed.processOutput
      : null;

  return (
    <Panel>
      {processes.length === 0 ? (
        <div className="panel-empty">{t("proc.empty")}</div>
      ) : (
        <div className="proc-list">
          {processes.map((process) => (
            <ProcessRow
              key={process.id}
              process={process}
              selected={process.id === watched}
              onSelect={() => onWatch(process.id)}
            />
          ))}
        </div>
      )}

      {selected && (
        <div className="proc-output">
          <div className="proc-output-head">
            {output !== null && output.dropped > 0 && (
              <span className="proc-dropped">{tp("proc.dropped", output.dropped)}</span>
            )}
            <div className="composer-gap" />
            <StopOrCopy
              running={selected.state.type === "running"}
              text={output?.text ?? ""}
              onStop={() => onStop(selected.id)}
            />
          </div>
          <pre className="proc-output-text">
            {output?.text || t("row.no_output")}
          </pre>
        </div>
      )}
    </Panel>
  );
}

function ProcessRow({
  process,
  selected,
  onSelect,
}: {
  process: ProcessSummary;
  selected: boolean;
  onSelect: () => void;
}) {
  const ended = process.finishedAt ?? Date.now();
  return (
    <button
      className="proc"
      aria-pressed={selected}
      onClick={onSelect}
    >
      <span className="proc-num">#{process.id}</span>
      <span className="proc-cmd">{process.command}</span>
      <ProcessStateView state={process.state} />
      <span className="proc-time">{durationOf(ended - process.startedAt)}</span>
    </button>
  );
}

function ProcessStateView({ state }: { state: ProcessState }) {
  if (state.type === "running") {
    return (
      <span className="proc-state proc-state-run">
        <span className="pulse"><Circle size={7} weight="fill" /></span> {t("proc.state_running")}
      </span>
    );
  }
  if (state.type === "stopped") {
    return <span className="proc-state">{t("proc.state_stopped")}</span>;
  }
  const failed = state.code !== null && state.code !== 0;
  return (
    <span className={failed ? "proc-state proc-state-fail" : "proc-state proc-state-ok"}>
      {t("proc.state_exited", { code: state.code ?? "—" })}
    </span>
  );
}

function StopOrCopy({
  running,
  text,
  onStop,
}: {
  running: boolean;
  text: string;
  onStop: () => void;
}) {
  const { copied, copy } = useCopy();
  return (
    <>
      {running ? (
        <button className="btn btn-small" onClick={onStop}>
          {t("proc.stop")}
        </button>
      ) : (
        <button className="btn btn-small" onClick={() => copy(text)}>
          {copied ? t("feed.copied") : t("feed.copy")}
        </button>
      )}
    </>
  );
}
