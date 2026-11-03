/**
 * What the chat is for (§7.1): code, or documents. Two items in one list.
 *
 * Not a level of trust, so not part of the mode menu: the modes say how far
 * Yaca may go, and this says what the work is. A chat about a report still
 * asks before it writes or runs anything, and a cautious chat about code is
 * exactly what "Manual" is for.
 *
 * The chip stands before the mode chip, in the order the row is read: what I am
 * doing, then how far you may go.
 */

import { Check } from "@phosphor-icons/react";
import { useEffect } from "react";

import type { Profile } from "../core/client";
import { t, type Key } from "../i18n";
import type { ComposerActions, ComposerSettings } from "./Composer";

/** In the order the list shows them: the default first. */
export const PROFILES: Profile[] = ["code", "chat"];

export const PROFILE_KEY: Record<Profile, Key> = {
  code: "profile.code",
  chat: "profile.chat",
};
const PROFILE_NOTE: Record<Profile, Key> = {
  code: "profile.code_note",
  chat: "profile.chat_note",
};

export function ProfileMenu({
  settings,
  actions,
  onClose,
}: {
  settings: ComposerSettings;
  actions: ComposerActions;
  onClose: () => void;
}) {
  // The digits belong to the open list while it is on screen, as in the mode
  // menu: `1` and `2` must do what is written next to them.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const at = Number(event.key) - 1;
      if (!Number.isInteger(at) || at < 0 || at >= PROFILES.length) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      actions.setProfile(PROFILES[at]);
      onClose();
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [actions, onClose]);

  return (
    <div className="menu menu-notes">
      <div className="menu-item" style={{ color: "var(--muted)" }}>
        {t("profile.title")}
      </div>
      {PROFILES.map((profile, index) => (
        <button
          className="menu-item"
          key={profile}
          onClick={() => {
            actions.setProfile(profile);
            onClose();
          }}
        >
          <span className="menu-text">
            <span>{t(PROFILE_KEY[profile])}</span>
            <span className="menu-note">{t(PROFILE_NOTE[profile])}</span>
          </span>
          {settings.profile === profile && (
            <span className="menu-mark">
              <Check size={13} weight="bold" />
            </span>
          )}
          <span className="keycap">{index + 1}</span>
        </button>
      ))}
    </div>
  );
}
