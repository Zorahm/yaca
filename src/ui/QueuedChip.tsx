/**
 * The chip above the composer for a message that waits for the agent (§7.4):
 * `в очереди · <text> · отменить`.
 *
 * One per queued message, in the order the core holds them. The chip is the
 * core's list drawn as it is: it appears and goes when the core says so, never
 * because the interface guessed. `отменить` is a real button, so it is reached
 * with `Tab` and pressed with `Enter`, and it sends the cancel with this
 * message's id; the core answers with the list that results.
 */

import { t } from "../i18n";

export function QueuedChip({
  text,
  onCancel,
}: {
  text: string;
  /** Absent where there is no session to cancel in: the chip then says only what it is. */
  onCancel?: () => void;
}) {
  return (
    <div className="queued">
      <span>{t("composer.queued")}</span>
      <span className="mono-target">{text}</span>
      {onCancel !== undefined && (
        <>
          <span aria-hidden="true">·</span>
          <button className="queued-cancel" onClick={onCancel}>
            {t("composer.queued_cancel")}
          </button>
        </>
      )}
    </div>
  );
}
