/**
 * Панель чекпоинтов §1.1: список точек этого прогона и дифф от выбранной.
 *
 * Список ядро отдаёт целиком событием `checkpointsUpdated` — панель есть
 * функция от него, своего списка тут нет. То же и с диффом: выбор строки шлёт
 * `previewCheckpoint`, ядро отвечает `checkpointPreviewed`, и показанное —
 * ровно то, что приехало. Пересчитать его в окне было бы второй версией
 * правды, а расходится она ровно тогда, когда дерево двинулось между вопросом
 * и ответом — то есть когда цена ошибки наибольшая.
 *
 * Свежие сверху: панель — способ вернуться назад, а возвращаются обычно
 * недалеко. Лента идёт в обратном порядке, но там точка стоит на своём месте
 * в разговоре, а здесь она строка списка.
 *
 * Дифф — тот же список файлов, что показывает подтверждение §5.8: что откат
 * сделает с каждым. Построчного диффа от точки ядро не отдаёт, и рисовать его
 * тут не из чего.
 *
 * Точки прошлых прогонов сюда не попадают: их список живёт в памяти сессии и
 * не переживает её (README). Открытая заново сессия начинает с пустой панели,
 * и это правда — вернуться к вчерашней точке она не может.
 */

import { Circle } from "@phosphor-icons/react";

import type { CheckpointChange, CheckpointSummary } from "../../core/client";
import type { Feed } from "../../core/feed";
import { t } from "../../i18n";
import { reasonText, willDo } from "../checkpoint";
import { timeOf } from "../format";
import { Panel } from "./Panel";

export function CheckpointsPanel({
  feed,
  onClose,
  onPreview,
  onRestore,
  onCancel,
}: {
  feed: Feed;
  onClose: () => void;
  onPreview: (id: string) => void;
  onRestore: (id: string) => void;
  /** Снять вопрос, ничего не откатывая. */
  onCancel: () => void;
}) {
  const points = [...feed.checkpoints].reverse();
  const preview = feed.preview;

  return (
    <Panel open onOpenChange={(next) => !next && onClose()} title={t("top.checkpoints")}>
      {points.length === 0 ? (
        <div className="panel-empty">{t("ckpt.empty")}</div>
      ) : (
        <div className="ckpt-list">
          {points.map((point) => (
            <CheckpointRow
              key={point.id}
              point={point}
              asking={preview?.id === point.id}
              files={preview?.id === point.id ? preview.files : null}
              onPreview={() => onPreview(point.id)}
              onRestore={() => onRestore(point.id)}
              onCancel={onCancel}
            />
          ))}
        </div>
      )}
    </Panel>
  );
}

function CheckpointRow({
  point,
  asking,
  files,
  onPreview,
  onRestore,
  onCancel,
}: {
  point: CheckpointSummary;
  asking: boolean;
  files: CheckpointChange[] | null;
  onPreview: () => void;
  onRestore: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="ckpt">
      <button className="ckpt-row" aria-expanded={asking} onClick={onPreview}>
        <span className="ckpt-dot">
          <Circle size={6} weight="fill" />
        </span>
        <span className="ckpt-time">{timeOf(point.at)}</span>
        <span className="ckpt-reason">{reasonText(point.reason)}</span>
      </button>

      {/* §5.8: откат показывает, что тронет, и ждёт подтверждения — до того,
          как что-либо произошло. */}
      {asking && files !== null && (
        <div className="confirm">
          <div className="confirm-title">
            {t("ckpt.confirm_title", { time: timeOf(point.at) })}
          </div>
          {files.length === 0 ? (
            <div className="confirm-list">{t("ckpt.confirm_nothing")}</div>
          ) : (
            <div className="confirm-list">
              {files.map((file) => (
                <div className="confirm-file" key={file.path}>
                  <span>{file.path}</span>
                  <span className="confirm-what">{willDo(file)}</span>
                </div>
              ))}
            </div>
          )}
          <div className="error-actions">
            <button className="btn" onClick={onCancel}>
              {t("ckpt.confirm_no")}
            </button>
            <button className="btn btn-accent" onClick={onRestore}>
              {t("ckpt.confirm_yes")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
