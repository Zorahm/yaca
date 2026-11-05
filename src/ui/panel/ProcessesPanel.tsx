/** Background commands are separate disclosures; only the open output is watched. */
import { CaretDown, CaretRight, Circle } from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";

import type { ProcessId, ProcessState, ProcessSummary } from "../../core/client";
import { processTitles } from "../../core/backgroundCommands";
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
  watched: ProcessId | null;
  onWatch: (id: ProcessId | null) => void;
  onStop: (id: ProcessId) => void;
}) {
  const running = feed.processes.some((process) => process.state.type === "running");
  const titles = useMemo(() => processTitles(feed), [feed.blocks]);
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => setTick((n) => n + 1), 1000);
    return () => window.clearInterval(timer);
  }, [running]);

  return (
    <Panel>
      {feed.processes.length === 0 ? (
        <div className="panel-empty">{t("proc.empty")}</div>
      ) : (
        <div className="proc-list">
          {feed.processes.map((process) => (
            <ProcessCard
              key={process.id}
              process={process}
              title={titles.get(process.id)}
              open={process.id === watched}
              output={feed.processOutput?.id === process.id ? feed.processOutput : null}
              onToggle={() => onWatch(watched === process.id ? null : process.id)}
              onStop={() => onStop(process.id)}
            />
          ))}
        </div>
      )}
    </Panel>
  );
}

function ProcessCard({
  process,
  title,
  open,
  output,
  onToggle,
  onStop,
}: {
  process: ProcessSummary;
  title?: string;
  open: boolean;
  output: Feed["processOutput"];
  onToggle: () => void;
  onStop: () => void;
}) {
  const { copied, copy } = useCopy();
  const running = process.state.type === "running";
  const outputId = `process-output-${process.id}`;
  return (
    <article className="proc">
      <div className="proc-info">
        <div className="proc-heading">
          {title && <div className="proc-title">{title}</div>}
          <span className="proc-num">#{process.id}</span>
        </div>
        <code className="proc-cmd">{process.command}</code>
        {process.cwd !== "." && <div className="proc-cwd">{process.cwd}</div>}
        <div className="proc-meta">
          <ProcessStateView state={process.state} />
          <span className="proc-time">
            {durationOf((process.finishedAt ?? Date.now()) - process.startedAt)}
          </span>
          {running && (
            <button className="btn btn-row proc-stop" onClick={onStop}>{t("proc.stop")}</button>
          )}
        </div>
      </div>
      <button className="proc-toggle" aria-expanded={open} aria-controls={outputId} onClick={onToggle}>
        {open ? <CaretDown size={12} /> : <CaretRight size={12} />}
        {t(open ? "proc.hide" : "proc.watch")}
      </button>
      {open && (
        <div className="proc-output" id={outputId}>
          <div className="proc-output-head">
            {output !== null && output.dropped > 0 && (
              <span className="proc-dropped">{tp("proc.dropped", output.dropped)}</span>
            )}
            <div className="composer-gap" />
            <button className="btn btn-row" disabled={!output?.text} onClick={() => copy(output?.text ?? "")}>
              {t(copied ? "feed.copied" : "feed.copy")}
            </button>
          </div>
          <pre className="proc-output-text">
            {output === null ? t("proc.loading") : output.text || t("row.no_output")}
          </pre>
        </div>
      )}
    </article>
  );
}

function ProcessStateView({ state }: { state: ProcessState }) {
  if (state.type === "running") {
    return (
      <span className="proc-state proc-state-run">
        <span className="pulse"><Circle size={7} weight="fill" /></span>
        {t("proc.state_running")}
      </span>
    );
  }
  if (state.type === "stopped") return <span className="proc-state">{t("proc.state_stopped")}</span>;
  if (state.code === 0) return <span className="proc-state proc-state-ok">{t("proc.state_completed")}</span>;
  return <span className="proc-state proc-state-fail">{t("proc.state_failed", { code: state.code ?? "—" })}</span>;
}
