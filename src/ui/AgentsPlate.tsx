/**
 * Subagents in the feed: one plate for the agents sent together.
 *
 * It says how many there are, how many are still at work and what they have
 * spent, and it is a door: pressing it opens the agents panel on these
 * agents. What each of them did is not drawn here — a subagent's twenty reads
 * are exactly what it was sent to keep out of the conversation.
 *
 * The plate reads the `Task` rows and nothing else, so it does not assume the
 * turn is waiting for them: agents that outlive the stretch that sent them
 * would be drawn the same way.
 */

import { ArrowRight, Robot } from "@phosphor-icons/react";

import { tally } from "../core/agents";
import type { ActionRow } from "../core/feed";
import { t, tp } from "../i18n";
import { tokens } from "./format";
import { Spinner } from "./RowSign";

export function AgentsPlate({
  rows,
  onOpen,
}: {
  rows: ActionRow[];
  /** Opens the agents panel on these agents, by their `Task` call ids. */
  onOpen: (ids: string[]) => void;
}) {
  const sum = tally(rows);

  return (
    <button
      className="agents-plate"
      title={t("agents.open")}
      onClick={() => onOpen(rows.map((row) => row.callId))}
    >
      {sum.running > 0 ? (
        <Spinner />
      ) : (
        <span className="sign" aria-hidden="true">
          <Robot size={15} />
        </span>
      )}
      <span className="agents-plate-title">{tp("agents.count", sum.count)}</span>
      {sum.running > 0 && (
        <span className="agents-plate-note">{tp("agents.running", sum.running)}</span>
      )}
      {sum.failed > 0 && (
        <span className="agents-plate-note agents-plate-failed">
          {tp("agents.failed", sum.failed)}
        </span>
      )}
      {sum.tokens !== null && (
        <span className="agents-plate-note">
          {t("agents.tokens", { n: tokens(sum.tokens) })}
        </span>
      )}
      <span className="caret" aria-hidden="true">
        <ArrowRight size={12} />
      </span>
    </button>
  );
}
