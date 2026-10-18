/**
 * The words of a checkpoint §5.8 — what the point is called and what a rollback
 * will do to the file.
 *
 * The core names the reason as a structure (`CheckpointReason`), and the
 * interface writes the sentence — the same rule as the action caption §5.4.1. It
 * lives apart from the feed because there are two readers: the divider §5.8 and
 * the panel §1.1, and a second copy of these same words would drift from the
 * first at the first edit.
 */

import type { CheckpointChange, CheckpointSummary } from "../core/client";
import { t } from "../i18n";

/** `before a message`, `before npm test`, `before a rollback`. */
export function reasonText(reason: CheckpointSummary["reason"]): string {
  switch (reason.type) {
    case "message":
      return t("ckpt.message");
    case "command":
      return t("ckpt.command", { command: reason.command });
    case "rollback":
      return t("ckpt.rollback");
  }
}

/** What a rollback will do to this file — future tense: it has not happened yet. */
export function willDo(file: CheckpointChange): string {
  switch (file.kind) {
    case "created":
      return t("ckpt.will_create");
    case "deleted":
      return t("ckpt.will_delete");
    default:
      return t("ckpt.will_modify");
  }
}
