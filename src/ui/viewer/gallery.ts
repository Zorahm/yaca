/**
 * Все картинки разговора по порядку — то, что листает просмотрщик.
 *
 * И свои, и модели: приложенное человеком и открытое агентом через
 * `ReadImage`. Для листания разницы нет — это одна лента, и картинка, на
 * которую агент посмотрел в ответ на скриншот, стоит сразу за скриншотом.
 */

import { flatRows, type Feed } from "../../core/feed";

const IMAGE = /\.(png|jpe?g|gif|webp|bmp)$/i;

/**
 * Похож ли путь на картинку — по расширению.
 *
 * Только здесь, где спросить ядро нельзя: список для листания собирается из
 * ленты чистой функцией. Чем файл является на самом деле, решают байты, и
 * плитка спрашивает об этом ядро (`look`).
 */
export function isImagePath(path: string): boolean {
  return IMAGE.test(path);
}

export function galleryOf(feed: Feed | null): string[] {
  if (feed === null) return [];
  const out: string[] = [];
  const add = (path: string) => {
    // Один путь — одна остановка: скриншот, приложенный дважды, тот же файл.
    if (isImagePath(path) && !out.includes(path)) out.push(path);
  };

  for (const block of feed.blocks) {
    if (block.kind === "human") {
      block.attachments.forEach(add);
    } else if (block.kind === "actions") {
      for (const row of flatRows(block.rows)) {
        // Только то, что открылось: у упавшего вызова показывать нечего.
        if (row.tool === "ReadImage" && row.state === "done" && row.target) add(row.target);
      }
    }
  }
  return out;
}
