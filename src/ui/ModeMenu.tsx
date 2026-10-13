/**
 * The mode menu of the composer's bottom row (§7.1, §8): four mutually
 * exclusive modes in one list, with bypass as a flag on top of them.
 *
 * Plan sits in the same list as the three trust levels, not as an axis of its
 * own: it has no writing tools at all, so trust decides nothing while it is
 * on, and two controls would show a choice that does not exist.
 */

import { Check } from "@phosphor-icons/react";
import { useEffect } from "react";

import type { Mode } from "../core/client";
import { t, type Key } from "../i18n";
import type { ComposerActions, ComposerSettings } from "./Composer";

/**
 * In the order the list shows them, which is also the order `⇧⇥` walks: the
 * default first, plan last — it is the one that is not a level of trust.
 */
export const MODES: Mode[] = ["auto", "manual", "edits", "plan"];

export const MODE_KEY: Record<Mode, Key> = {
  plan: "mode.plan",
  manual: "mode.manual",
  auto: "mode.auto",
  edits: "mode.edits",
};
const MODE_NOTE: Record<Mode, Key> = {
  plan: "mode.plan_note",
  manual: "mode.manual_note",
  auto: "mode.auto_note",
  edits: "mode.edits_note",
};

export function ModeMenu({
  settings,
  actions,
  onClose,
}: {
  settings: ComposerSettings;
  actions: ComposerActions;
  onClose: () => void;
}) {
  const setBypass = actions.setBypass;

  // The digits belong to the open list, not to a permission request (§5.7):
  // while the list is on screen and shows its `1`–`4`, they must do what is
  // written. Hence the capture phase; the request's listener is on bubbling.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const at = Number(event.key) - 1;
      if (!Number.isInteger(at) || at < 0 || at >= MODES.length) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      actions.setMode(MODES[at]);
      onClose();
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [actions, onClose]);

  return (
    <div className="menu menu-notes">
      <div className="menu-item" style={{ color: "var(--muted)" }}>
        {t("mode.title")}
      </div>
      {MODES.map((mode, index) => (
        <button
          className="menu-item"
          key={mode}
          onClick={() => {
            actions.setMode(mode);
            onClose();
          }}
        >
          <span className="menu-text">
            <span>{t(MODE_KEY[mode])}</span>
            <span className="menu-note">{t(MODE_NOTE[mode])}</span>
          </span>
          {settings.mode === mode && <span className="menu-mark"><Check size={13} weight="bold" /></span>}
          <span className="keycap">{index + 1}</span>
        </button>
      ))}

      {setBypass && (
        <>
          <div className="menu-sep" />

          <button
            className="menu-item"
            onClick={() => {
              // Turning bypass on needs confirmation; no invariant is switched
              // off by it.
              if (settings.bypass || window.confirm(t("mode.bypass_confirm"))) {
                setBypass(!settings.bypass);
              }
              onClose();
            }}
          >
            <span className={settings.bypass ? "chip-danger" : undefined}>
              {t("mode.bypass")}
            </span>
            <span className="menu-note" />
            <span>
              {settings.bypass ? t("mode.bypass_off") : t("mode.bypass_on")}
            </span>
          </button>
        </>
      )}
    </div>
  );
}
