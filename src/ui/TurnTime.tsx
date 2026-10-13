/**
 * The clock of a turn: counting while it runs, and what it came to after.
 *
 * The figures come from the core's timestamps (`turnStarted.at` and the end
 * event's `at`), so a reopened chat shows what the live one did. Only the
 * ticking is the window's: the core sending an event every second would not
 * say anything new.
 */

import { useEffect, useState } from "react";

import { t } from "../i18n";
import { dateTimeOf, durationOf, timeOf } from "./format";

/** `Работал 2 мин 14 с · 13:28`; the full date is in the tooltip. */
export function TurnTime({ startedAt, endedAt }: { startedAt: number; endedAt: number }) {
  return (
    <div className="turn-time" title={dateTimeOf(endedAt)}>
      {t("turn.worked", { time: durationOf(endedAt - startedAt) })} · {timeOf(endedAt)}
    </div>
  );
}

/** `Работает 42 с` while the turn is running. */
export function TurnLive({ startedAt }: { startedAt: number }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  return (
    <div className="turn-time" title={dateTimeOf(startedAt)}>
      {t("turn.working", { time: durationOf(now - startedAt) })}
    </div>
  );
}
