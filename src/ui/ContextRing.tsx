/**
 * The ring under the composer (§7.1): the context window, filling up as the
 * window does. Hovered, it says how full; pressed, it opens into the parts
 * (`./ContextWindow`).
 *
 * It says nothing about the work. The send button turning into stop already
 * does, and a ring that also turned and pulsed was two readings on one mark.
 */

import type { UsageSnapshot } from "../core/client";
import { t } from "../i18n";
import { FULL_RATIO, shareOf } from "./contextParts";
import { percent, tokens } from "./format";

const R = 6.5;
const CIRCUMFERENCE = 2 * Math.PI * R;

export function ContextRing({ usage }: { usage: UsageSnapshot | null }) {
  const share = shareOf(usage);

  // Without a session there is no window to measure, and the ring is named
  // for what it will show.
  const label = usage
    ? t("ring.tooltip", {
        used: tokens(usage.contextUsed),
        max: tokens(usage.contextMax),
        share: percent(share, 0),
      })
    : t("ctx.title");

  return (
    <svg className="ring" viewBox="0 0 16 16" role="img" aria-label={label}>
      {/* The tooltip of an SVG is an element, not an attribute: §7.1 asks for
          a hover, and `<title>` inside is read by a screen reader too (§14). */}
      <title>{label}</title>
      <circle cx="8" cy="8" r={R} fill="none" stroke="var(--line-strong)" strokeWidth="2" opacity="0.45" />

      {share > 0 && (
        <circle
          className="ring-fill"
          cx="8"
          cy="8"
          r={R}
          fill="none"
          stroke={share >= FULL_RATIO ? "var(--warn)" : "var(--accent)"}
          strokeWidth="2"
          strokeLinecap="round"
          strokeDasharray={`${share * CIRCUMFERENCE} ${CIRCUMFERENCE}`}
          transform="rotate(-90 8 8)"
        />
      )}
    </svg>
  );
}
