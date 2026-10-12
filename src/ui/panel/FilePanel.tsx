/**
 * Панель файла §6.4: полный дифф на всю высоту с липкой шапкой.
 *
 * Открывается кликом по имени файла в строке действия (§1.1). Весь файл с
 * неизменёнными строками приходит от ядра (`fileViewLoaded`): в потоке лежит
 * только дифф одного вызова с тремя строками контекста, а §6.4 хочет файл целиком.
 * Между ханками — свёрнутые промежутки «⋯ N строк», их можно раскрыть.
 *
 * Панель только показывает. Решать про ханк здесь нечего: правка уже лежит в
 * рабочем дереве, и единственный путь назад — «Откатить сюда» на чекпоинте
 * (§5.8).
 */

import { ArrowBendUpLeft } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";

import type { Feed } from "../../core/feed";
import { factOf } from "../../core/feed";
import { t } from "../../i18n";
import { FileBadge } from "../FileBadge";
import { Diff, type FileBody } from "../Diff";
import { plainPath } from "../format";
import { Panel } from "./Panel";

export function FilePanel({
  path,
  feed,
  onClose,
  onRead,
}: {
  path: string;
  feed: Feed;
  onClose: () => void;
  /** Просит ядро отдать файл целиком (§6.4). */
  onRead: (path: string) => void;
}) {
  // Запрос уходит, когда панель показала другой файл и когда агент записал
  // файл ещё раз: `writes` растёт с каждой записью, и вид должен идти за ней.
  // Колбэк читается через ref, чтобы перерисовка родителя не
  // слала запрос заново — иначе ответ ядра и перерисовка кормили бы друг друга.
  const readRef = useRef(onRead);
  useEffect(() => {
    readRef.current = onRead;
  });
  useEffect(() => {
    readRef.current(path);
  }, [path, feed.writes]);

  // Раскрытые промежутки считаются по индексам сегментов одного вида: в новом
  // виде те же индексы значат другое, поэтому раскрытое сбрасывается.
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(() => new Set());
  const viewed = feed.fileView;
  useEffect(() => {
    setExpanded(new Set());
  }, [path, viewed]);

  // `undefined` — ядро ещё не ответило за этот файл; `null` — сессия этот
  // файл не меняла.
  const shown = viewed !== null && viewed.path === path ? viewed.view : undefined;
  const view = shown ?? null;
  const fact = view?.fact ?? factOf(feed, path);

  const body: FileBody | null =
    shown === null || shown === undefined
      ? null
      : {
          segments: shown.segments,
          truncated: shown.truncated,
          expanded,
          onExpand: (index) => setExpanded((now) => new Set(now).add(index)),
        };

  return (
    <Panel
      open
      onOpenChange={(next) => !next && onClose()}
      title={<span className="panel-title-mono">{plainPath(path)}</span>}
    >
      <div className="panel-file-head">
        {fact !== null && <FileBadge file={fact} />}
        {view !== null && (
          <span className="bar-counts">
            <span className="count-add">+{view.fact.added}</span>{" "}
            <span className="count-del">−{view.fact.removed}</span>
          </span>
        )}
        {fact?.movedFrom && (
          <span className="panel-file-moved"><ArrowBendUpLeft size={12} /> {fact.movedFrom}</span>
        )}
      </div>

      <div className="panel-file-diff">
        {shown === undefined && <div className="panel-empty">{t("file.loading")}</div>}
        {shown === null && <div className="panel-empty">{t("file.unchanged")}</div>}
        {body !== null && <Diff body={body} path={path} />}
      </div>
    </Panel>
  );
}

