/**
 * Панель файла §6.4: полный дифф на всю высоту с липкой шапкой.
 *
 * Открывается кликом по имени файла в строке действия (§1.1). Дифф и факт
 * приходят из потока (`diffOf`, `factOf`): последнее, что ядро прислало по
 * пути. Между ханками — свёрнутые промежутки «⋯ N строк», посчитанные из
 * нумерации старой стороны.
 *
 * Кнопок решения здесь нет: подтверждение спрашивается один раз и до записи
 * (§16.37), а файл на диске уже лежит.
 */

import { ArrowBendUpLeft } from "@phosphor-icons/react";

import type { Feed } from "../../core/feed";
import { diffOf, factOf } from "../../core/feed";
import { t } from "../../i18n";
import { FileBadge } from "../FileBadge";
import { Diff } from "../Diff";
import { plainPath } from "../format";
import { Panel } from "./Panel";

export function FilePanel({
  path,
  feed,
  onClose,
}: {
  path: string;
  feed: Feed;
  onClose: () => void;
}) {
  const diff = diffOf(feed, path);
  const fact = factOf(feed, path);
  // Счётчики есть и у факта, и у диффа: факт точнее, дифф — запасной.
  const counts = fact ?? diff;

  // Путь приводится только на показ: `diffOf` и `factOf` выше ищут по той
  // форме, что прислало ядро.
  return (
    <Panel
      open
      onOpenChange={(next) => !next && onClose()}
      title={<span className="panel-title-mono">{plainPath(path)}</span>}
    >
      <div className="panel-file-head">
        {fact !== null && <FileBadge file={fact} />}
        {counts !== null && (
          <span className="bar-counts">
            <span className="count-add">+{counts.added}</span>{" "}
            <span className="count-del">−{counts.removed}</span>
          </span>
        )}
        {fact?.movedFrom && (
          <span className="panel-file-moved"><ArrowBendUpLeft size={12} /> {fact.movedFrom}</span>
        )}
      </div>

      <div className="panel-file-diff">
        {diff ? (
          <Diff diff={diff} gaps />
        ) : (
          <div className="panel-empty">{t("file.no_diff")}</div>
        )}
      </div>
    </Panel>
  );
}
