/**
 * Normalising a path into the form the UI shows it in (§5.5, §7.2).
 *
 * The file dialog returns an absolute path in the operating system's own form,
 * while the chip and the card show the path from the project root, the same one
 * the model sees. Separators and case differ between the two sides, and this
 * check is about exactly that.
 */

import { describe, expect, it } from "vitest";

import { baseName, relativeTo } from "./format";

describe("path from the project root", () => {
  it("strips the root regardless of the separator", () => {
    expect(relativeTo("D:\\Kod", "D:\\Kod\\src\\App.tsx")).toBe("src/App.tsx");
    expect(relativeTo("D:/Kod", "D:\\Kod\\src\\App.tsx")).toBe("src/App.tsx");
    expect(relativeTo("/home/z/kod", "/home/z/kod/src/App.tsx")).toBe("src/App.tsx");
  });

  it("ignores case: Windows does not distinguish it either", () => {
    expect(relativeTo("D:\\Kod", "d:\\kod\\src\\App.tsx")).toBe("src/App.tsx");
  });

  it("tolerates a trailing separator on the root", () => {
    expect(relativeTo("D:\\Kod\\", "D:\\Kod\\README.md")).toBe("README.md");
  });

  it("leaves a foreign path absolute", () => {
    // Lying about where a file lives is worse than showing it at length.
    expect(relativeTo("D:\\Kod", "C:\\Temp\\note.md")).toBe("C:/Temp/note.md");
    // A sibling whose name shares a prefix is not inside.
    expect(relativeTo("D:\\Kod", "D:\\Kodex\\note.md")).toBe("D:/Kodex/note.md");
  });

  it("does not count the root itself as a path inside it", () => {
    expect(relativeTo("D:\\Kod", "D:\\Kod")).toBe("D:/Kod");
  });
});

describe("file name", () => {
  it("splits on both separators", () => {
    expect(baseName("src/ui/App.tsx")).toBe("App.tsx");
    expect(baseName("D:\\Kod\\README.md")).toBe("README.md");
    expect(baseName("README.md")).toBe("README.md");
  });
});
