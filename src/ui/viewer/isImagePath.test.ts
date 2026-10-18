/**
 * Only what looks like a picture enters the viewer's paging: the list is built from the feed by
 * a pure function, and it has no way to ask the core about each path.
 */

import { describe, expect, it } from "vitest";

import { isImagePath } from "./gallery";

describe("isImagePath", () => {
  it("recognises a picture by its extension, however it is typed", () => {
    for (const path of ["shot.png", "a/b/Photo.JPG", "x.jpeg", "anim.gif", "p.webp", "old.BMP"]) {
      expect(isImagePath(path), path).toBe(true);
    }
  });

  it("does not take anything else for a picture", () => {
    // SVG is text, and `Read` reads it; `png` in the middle of a name means nothing.
    for (const path of ["src/a.rs", "logo.svg", "png.txt", "shot.png.bak", "png"]) {
      expect(isImagePath(path), path).toBe(false);
    }
  });
});
