/**
 * The model chip of the bottom row and its list (§7.1).
 *
 * The list holds every provider's models, each bringing its own provider. A
 * model a local server has not loaded is marked: picking it costs a long load
 * or an error, and a list that looks the same for all of them hides that
 * (`./residency`).
 */

import { Check } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { modelResidency, type ModelInfo } from "../core/client";
import { t } from "../i18n";
import { createResidency, residencyKey, type Residency } from "./residency";

/** One per window: which providers stay quiet is worth remembering all run. */
const residencyOf = createResidency(modelResidency);

export function ModelMenu({
  models,
  active,
  model,
  provider,
  open,
  onToggle,
  onPick,
}: {
  models: ModelInfo[];
  /** What is known of the chosen model; `undefined` when no provider lists it. */
  active: ModelInfo | undefined;
  model: string;
  provider?: string;
  open: boolean;
  onToggle: () => void;
  onPick: (model: ModelInfo) => void;
}) {
  const [residency, setResidency] = useState<Residency>(new Map());

  // A string, not the array: the list arrives as a fresh array on every render
  // of the window, and each one would ask the servers again.
  const providers = [...new Set(models.map((m) => m.providerId))].join("\n");

  useEffect(() => {
    if (!open) return;
    let live = true;
    // Last open's marks may be stale already; none is better than wrong ones.
    setResidency(new Map());
    void residencyOf(providers.split("\n")).then((marks) => {
      if (live) setResidency(marks);
    });
    return () => {
      live = false;
    };
  }, [open, providers]);

  return (
    <div className="menu-anchor">
      <button className="chip chip-strong" onClick={onToggle}>
        {active?.displayName ?? (model || t("composer.no_model"))}
      </button>
      {open && (
        <div className="menu">
          {models.map((m) => {
            const unloaded = residency.get(residencyKey(m.providerId, m.id)) === false;
            return (
              <button
                className="menu-item"
                key={`${m.providerId || ""}:${m.id}`}
                onClick={() => onPick(m)}
              >
                <span>{m.displayName}</span>
                <span className="menu-note">
                  {m.providerId || ""}
                  {unloaded && ` · ${t("composer.model_unloaded")}`}
                </span>
                {m.id === model &&
                  (!provider || !m.providerId || m.providerId === provider) && (
                    <span className="menu-mark">
                      <Check size={13} weight="bold" />
                    </span>
                  )}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
