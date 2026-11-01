/**
 * The clock of a turn: counting while it runs, and what it came to after.
 *
 * The figures come from the core's timestamps (`turnStarted.at` and the end
 * event's `at`), so a reopened chat shows what the live one did. Only the
 * ticking is the window's: the core sending an event every second would not
 * say anything new.
 */

import { useEffect, useState, type ReactNode } from "react";
import type { ProcessSummary } from "../core/client";

import { t } from "../i18n";
import { dateTimeOf, durationOf, timeOf } from "./format";

/** `Worked 2 min 14 s · 13:28`; the full date is in the tooltip. */
export function TurnTime({ startedAt, endedAt, children }: { startedAt: number; endedAt: number; children?: ReactNode }) {
  return (
    <div className="turn-time" title={dateTimeOf(endedAt)}>
      {t("turn.worked", { time: durationOf(endedAt - startedAt) })} · {timeOf(endedAt)}
      {children}
    </div>
  );
}

/** `Working 42 s` while the turn is running. */
export function TurnLive({ startedAt, children }: { startedAt: number; children?: ReactNode }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  return (
    <div className="turn-time" title={dateTimeOf(startedAt)}>
      {t("turn.working", { time: durationOf(now - startedAt) })}
      {children}
    </div>
  );
}

export function ProcessStatus({ processes, onOpen }: { processes: ProcessSummary[]; onOpen: () => void }) {
  if (processes.length === 0) return null;
  const running = processes.filter((process) => process.state.type === "running").length;
  return <button className="feed-processes" onClick={onOpen}>
    {running > 0 && <span className="spinner" aria-hidden="true" />}
    {running > 0 ? t("proc.active_count", { n: running }) : t("proc.finished_count", { n: processes.length })}
  </button>;
}
