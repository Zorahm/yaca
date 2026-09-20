/**
 * The project chip above the composer (§9.1). A button, because §9 makes it
 * the way a project is changed.
 *
 * There is no plus for a second folder. The core carries one root through
 * staging, checkpoints and the permission broker, and a plus that replaces the
 * folder instead of adding one would lie with its icon (§0).
 */

import { Folder } from "@phosphor-icons/react";

import { t } from "../i18n";
import { baseName } from "./format";

export function ProjectChip({
  path,
  onPick,
  disabled,
}: {
  /** `null` before any project has been chosen — the chip asks for one. */
  path: string | null;
  onPick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      className="chip-context"
      onClick={onPick}
      disabled={disabled}
      title={path ?? undefined}
    >
      <span className="chip-icon" aria-hidden="true">
        <Folder size={13} />
      </span>
      {path === null ? t("home.pick_project") : baseName(path)}
    </button>
  );
}
