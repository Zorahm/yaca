/**
 * Приведение пути к виду, в котором его показывают (§5.5, §7.2).
 *
 * Диалог файлов отдаёт абсолютный путь в форме операционной системы, а чип и
 * карточка показывают путь от корня — тот же, что видит модель. Разделители
 * и регистр у двух сторон разные, и на этом проверка и держится.
 */

import { describe, expect, it } from "vitest";

import { baseName, relativeTo } from "./format";

describe("путь от корня проекта", () => {
  it("срезает корень независимо от разделителя", () => {
    expect(relativeTo("D:\\Kod", "D:\\Kod\\src\\App.tsx")).toBe("src/App.tsx");
    expect(relativeTo("D:/Kod", "D:\\Kod\\src\\App.tsx")).toBe("src/App.tsx");
    expect(relativeTo("/home/z/kod", "/home/z/kod/src/App.tsx")).toBe("src/App.tsx");
  });

  it("не различает регистр: Windows его тоже не различает", () => {
    expect(relativeTo("D:\\Kod", "d:\\kod\\src\\App.tsx")).toBe("src/App.tsx");
  });

  it("терпит хвостовой разделитель у корня", () => {
    expect(relativeTo("D:\\Kod\\", "D:\\Kod\\README.md")).toBe("README.md");
  });

  it("оставляет чужой путь абсолютным", () => {
    // Соврать про то, где лежит файл, хуже, чем показать его длинно.
    expect(relativeTo("D:\\Kod", "C:\\Temp\\note.md")).toBe("C:/Temp/note.md");
    // Сосед с общим началом имени — не внутри.
    expect(relativeTo("D:\\Kod", "D:\\Kodex\\note.md")).toBe("D:/Kodex/note.md");
  });

  it("не считает сам корень путём внутри него", () => {
    expect(relativeTo("D:\\Kod", "D:\\Kod")).toBe("D:/Kod");
  });
});

describe("имя файла", () => {
  it("режет по обоим разделителям", () => {
    expect(baseName("src/ui/App.tsx")).toBe("App.tsx");
    expect(baseName("D:\\Kod\\README.md")).toBe("README.md");
    expect(baseName("README.md")).toBe("README.md");
  });
});
