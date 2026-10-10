/**
 * Картинки в композере: вставка из буфера и перетаскивание в окно (§7.2).
 *
 * Отдельно от экранов, потому что композер один, а экранов два — сессия и
 * домашний, — и оба дают ему одно и то же: корень проекта, от которого
 * считаются пути.
 */

import { hasFileDialogs, onFilesDropped, stashImage } from "../core/client";
import type { ComposerActions } from "./Composer";
import { relativeTo } from "./format";

type ImageActions = Pick<ComposerActions, "stashImage" | "onDropped">;

/**
 * Один набор на корень. Композер подписывается на перетаскивание эффектом,
 * и действие, которое на каждой перерисовке новое, переподписывало бы его
 * на каждой перерисовке.
 */
const made = new Map<string, ImageActions>();

/** Без проекта и без оболочки действий нет — и вставка остаётся обычной. */
export function imageActions(root: string | null): ImageActions {
  if (root === null || !hasFileDialogs) return {};
  let actions = made.get(root);
  if (actions === undefined) {
    actions = {
      stashImage: (data) => stashImage(root, data),
      // Пути от корня, как у `＋`: чип и карточка §5.1 показывают то же, что
      // видит модель. Файл снаружи проекта остаётся абсолютным.
      onDropped: (listener) =>
        onFilesDropped((paths) => listener(paths.map((path) => relativeTo(root, path)))),
    };
    made.set(root, actions);
  }
  return actions;
}
