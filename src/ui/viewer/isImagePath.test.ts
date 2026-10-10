/**
 * В листание просмотрщика попадает только то, что похоже на картинку: список
 * собирается из ленты чистой функцией, и спросить ядро про каждый путь ей
 * нечем.
 */

import { describe, expect, it } from "vitest";

import { isImagePath } from "./gallery";

describe("isImagePath", () => {
  it("узнаёт картинку по расширению, как бы оно ни было набрано", () => {
    for (const path of ["shot.png", "a/b/Photo.JPG", "x.jpeg", "anim.gif", "p.webp", "old.BMP"]) {
      expect(isImagePath(path), path).toBe(true);
    }
  });

  it("не принимает за картинку остальное", () => {
    // SVG — текст, и читает его `Read`; `png` в середине имени ничего не значит.
    for (const path of ["src/a.rs", "logo.svg", "png.txt", "shot.png.bak", "png"]) {
      expect(isImagePath(path), path).toBe(false);
    }
  });
});
