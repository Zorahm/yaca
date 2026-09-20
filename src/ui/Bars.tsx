/**
 * Полосы уведомлений §6.1.1.
 *
 * Полосы изменений здесь больше нет: правки уезжают в рабочее дерево сразу, а
 * подтверждение спрашивается один раз, до записи (§8, §16.37). Полоса с
 * кнопкой «Применить» была вторым подтверждением того же самого.
 *
 * Полос уведомлений здесь одна из пяти, перечисленных в §6.1.1, — та, про
 * которую интерфейсу есть откуда узнать. Полоса «LSP не отвечает» без языкового
 * сервера, «файл изменён извне» без наблюдателя за деревом и «доступна версия»
 * без обновлятора — это надписи, которые никогда не появятся, и место, где
 * потом никто не вспомнит, что они не работают.
 *
 * There is no "no API key" bar. A key is not required up front (§10): a
 * provider that wants one refuses the request, and the error names the key.
 * A warning ahead of that was wrong for every local server, which never asks.
 */

import { Warning, X } from "@phosphor-icons/react";
import { useState } from "react";

import type { Feed } from "../core/feed";
import { t } from "../i18n";
import { tokens } from "./format";

/** §6.1.1: полоса появляется, когда контекст занят на 80%. */
const FULL_RATIO = 0.8;

export function Notices({
  feed,
  onCompact,
}: {
  feed: Feed;
  onCompact: () => void;
}) {
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());
  const hide = (id: string) => setDismissed(new Set(dismissed).add(id));

  const usage = feed.usage;
  const full =
    usage !== null &&
    usage.contextMax > 0 &&
    usage.contextUsed / usage.contextMax >= FULL_RATIO;

  return (
    <>
      {full && !dismissed.has("context") && (
        <div className="bar">
          <span className="bar-icon"><Warning size={15} /></span>
          <span className="bar-note">
            {t("notice.context_full")} · {tokens(usage.contextUsed)}/
            {tokens(usage.contextMax)}
          </span>
          <button className="btn btn-small" onClick={onCompact}>
            {t("notice.compact")}
          </button>
          <button
            className="bar-close"
            onClick={() => hide("context")}
            aria-label={t("notice.dismiss")}
          >
            <X size={14} />
          </button>
        </div>
      )}
    </>
  );
}
