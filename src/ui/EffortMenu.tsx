/** The effort chip of the bottom row and its list (§7.1). */

import { Check } from "@phosphor-icons/react";

import type { Effort } from "../core/client";
import { t, type Key } from "../i18n";

const EFFORTS: Effort[] = ["low", "medium", "high", "extra", "max"];

const EFFORT_KEY: Record<Effort, Key> = {
  low: "effort.low",
  medium: "effort.medium",
  high: "effort.high",
  extra: "effort.extra",
  max: "effort.max",
};

export function EffortMenu({
  effort,
  open,
  onToggle,
  onPick,
}: {
  effort: Effort;
  open: boolean;
  onToggle: () => void;
  onPick: (effort: Effort) => void;
}) {
  return (
    <div className="menu-anchor">
      <button className="chip" onClick={onToggle}>
        {t(EFFORT_KEY[effort])}
      </button>
      {open && (
        <div className="menu">
          {EFFORTS.map((level) => (
            <button className="menu-item" key={level} onClick={() => onPick(level)}>
              <span>{t(EFFORT_KEY[level])}</span>
              <span className="menu-note" />
              {level === effort && (
                <span className="menu-mark">
                  <Check size={13} weight="bold" />
                </span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
