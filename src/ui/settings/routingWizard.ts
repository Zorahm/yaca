/**
 * The quick setup of the router (§10): three models, one ready configuration.
 *
 * It writes into the editor and not to the disk. The text still goes through
 * the core's check on Save, and an existing file is not replaced behind the
 * person's back by a button that looks like "fill in the form".
 *
 * The configuration is deliberately one question and one rule. That covers the
 * case the router exists for — a strong model for what needs it, a cheap one
 * for the rest — and anything more is the editor's, where the whole language is.
 */

import type { ModelInfo } from "../../core/client";
import { modelKey } from "../modelSearch";

/** The "not chosen" value of a list: `Select` does not take an empty string. */
export const NO_MODEL = "-";

/** What the person picked, as `modelKey`s. */
export type Picks = {
  /** Answers the questions about each message: small and fast. */
  decider: string;
  /** Takes everything no rule claimed, and every message when routing fails. */
  fallback: string;
  /** Takes what the question says is hard. */
  strong: string;
};

export const emptyPicks: Picks = { decider: NO_MODEL, fallback: NO_MODEL, strong: NO_MODEL };

/**
 * How long the decision model has to answer. A hosted small model needs more
 * than the core's default second to start, and a timeout is silent: every
 * message would go to the fallback and the router would look like it worked.
 */
const TIMEOUT_MS = 3000;

/**
 * The configuration text for the picks, or `null` while one is missing or names
 * a model the list no longer has.
 */
export function configText(models: ModelInfo[], picks: Picks): string | null {
  const find = (key: string) => models.find((m) => modelKey(m) === key);
  const decider = find(picks.decider);
  const fallback = find(picks.fallback);
  const strong = find(picks.strong);
  if (decider === undefined || fallback === undefined || strong === undefined) return null;

  const ref = (m: ModelInfo) => ({ provider: m.providerId, model: m.id });
  return JSON.stringify(
    {
      name: "auto",
      decisionModel: { ...ref(decider), timeoutMs: TIMEOUT_MS },
      default: ref(fallback),
      signals: [
        {
          name: "needs_reasoning",
          question: {
            type: "noul",
            instructions: "Does answering this request need multi-step reasoning?",
          },
        },
      ],
      decisions: [
        {
          name: "hard",
          priority: 100,
          onUnknown: "noMatch",
          rules: { signal: { name: "needs_reasoning", predicate: { gte: 0.7 } } },
          model: ref(strong),
        },
      ],
    },
    null,
    2,
  );
}
