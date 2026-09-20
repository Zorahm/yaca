/**
 * Слова чекпоинта §5.8 — как называется точка и что откат сделает с файлом.
 *
 * Ядро называет причину структурой (`CheckpointReason`), а предложение пишет
 * интерфейс — то же правило, что у подписи действия §5.4.1. Лежит отдельно от
 * ленты, потому что читателей двое: разделитель §5.8 и панель §1.1, и второй
 * экземпляр этих же слов разошёлся бы с первым на первой же правке.
 */

import type { CheckpointChange, CheckpointSummary } from "../core/client";
import { t } from "../i18n";

/** `перед сообщением`, `перед npm test`, `перед откатом`. */
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

/** Что откат сделает с этим файлом — будущее время: он ещё не случился. */
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
