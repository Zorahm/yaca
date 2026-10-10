import { describe, expect, it } from "vitest";

import {
  MAX_SCALE,
  actual,
  clamp,
  fitScale,
  fitted,
  hits,
  isClick,
  isFitted,
  panned,
  percent,
  turned,
  zoomAt,
  type Size,
} from "./zoom";

const box: Size = { width: 1000, height: 600 };
const wide: Size = { width: 4000, height: 1200 };
const icon: Size = { width: 64, height: 64 };

describe("просмотрщик: масштаб", () => {
  it("большая картинка открывается целиком и по центру", () => {
    const view = fitted(wide, box);
    expect(view).toEqual({ scale: 0.25, x: 0, y: 0, turns: 0 });
    expect(isFitted(view, wide, box)).toBe(true);
  });

  it("маленькую не растягивает на всё окно", () => {
    // От увеличения иконка подробнее не станет.
    expect(fitScale(icon, box, 0)).toBe(1);
  });

  it("то, что под курсором, остаётся под курсором", () => {
    const start = fitted(wide, box);
    const at = { x: 300, y: -100 };
    // Точка картинки под курсором: (at - сдвиг) / масштаб.
    const before = { x: (at.x - start.x) / start.scale, y: (at.y - start.y) / start.scale };

    // В восемь раз: картинка больше рамки по обеим осям, и ни одна из них
    // не прижимает её к центру.
    const view = zoomAt(start, 8, at, wide, box);

    expect(view.scale).toBe(2);
    expect((at.x - view.x) / view.scale).toBeCloseTo(before.x);
    expect((at.y - view.y) / view.scale).toBeCloseTo(before.y);
  });

  it("не уходит мельче, чем «целиком», и крупнее предела", () => {
    const start = fitted(wide, box);
    expect(zoomAt(start, 0.1, { x: 0, y: 0 }, wide, box).scale).toBe(0.25);
    expect(zoomAt(start, 1000, { x: 0, y: 0 }, wide, box).scale).toBe(MAX_SCALE);
  });

  it("отдаление возвращает картинку в центр", () => {
    // Иначе, отъехав от угла, получаешь картинку, прижатую к краю окна.
    const corner = zoomAt(fitted(wide, box), 8, { x: 400, y: 250 }, wide, box);
    const back = zoomAt(corner, 1 / 8, { x: 400, y: 250 }, wide, box);
    expect(isFitted(back, wide, box)).toBe(true);
  });

  it("«точка в точку» — это сто процентов", () => {
    expect(percent(actual(fitted(wide, box), wide, box))).toBe(100);
  });
});

describe("просмотрщик: сдвиг", () => {
  it("картинку меньше рамки сдвинуть нельзя", () => {
    const view = fitted(icon, box);
    expect(panned(view, 200, 200, icon, box)).toEqual(view);
  });

  it("край большой картинки не уходит внутрь рамки", () => {
    const view = actual(fitted(wide, box), wide, box);
    const far = panned(view, 99_999, 99_999, wide, box);
    // 4000 при ширине рамки 1000: по полторы тысячи в каждую сторону.
    expect(far.x).toBe(1500);
    expect(far.y).toBe(300);
    expect(panned(far, -99_999 * 2, 0, wide, box).x).toBe(-1500);
  });

  it("по оси, где картинка помещается, она стоит по центру", () => {
    // Вдвое крупнее «целиком»: ширина 2000 не помещается, высота 600 — ровно.
    const view = clamp({ scale: 0.5, x: 300, y: 300, turns: 0 }, wide, box);
    expect(view.x).toBe(300);
    expect(view.y).toBe(0);
  });
});

describe("просмотрщик: поворот", () => {
  it("повёрнутая набок вписывается по другой стороне", () => {
    const view = turned(fitted(wide, box), wide, box);
    // Теперь это 1200×4000 в рамке 1000×600.
    expect(view.turns).toBe(1);
    expect(view.scale).toBeCloseTo(600 / 4000);
    expect(view.x).toBe(0);
  });

  it("четыре поворота возвращают как было", () => {
    let view = fitted(wide, box);
    for (let i = 0; i < 4; i += 1) view = turned(view, wide, box);
    expect(view).toEqual(fitted(wide, box));
  });
});

describe("просмотрщик: щелчок по пустому месту", () => {
  it("у вписанной картинки пустое место — поля вокруг", () => {
    // 4000×1200 в рамке 1000×600: полоса 1000×300 по центру.
    const view = fitted(wide, box);
    expect(hits(view, wide, { x: 0, y: 0 })).toBe(true);
    expect(hits(view, wide, { x: 499, y: 149 })).toBe(true);
    expect(hits(view, wide, { x: 0, y: 200 })).toBe(false);
    expect(hits(view, wide, { x: 0, y: -200 })).toBe(false);
  });

  it("увеличенная во всю рамку пустого места не оставляет", () => {
    const view = actual(fitted(wide, box), wide, box);
    for (const at of [
      { x: -500, y: -300 },
      { x: 500, y: 300 },
      { x: 0, y: 0 },
    ]) {
      expect(hits(view, wide, at)).toBe(true);
    }
  });

  it("сдвинутая картинка уносит своё место с собой", () => {
    // Иконка 64×64 по центру; после сдвига её нет там, где она была.
    const moved = { ...fitted(icon, box), x: 200, y: 0 };
    expect(hits(moved, icon, { x: 0, y: 0 })).toBe(false);
    expect(hits(moved, icon, { x: 210, y: 10 })).toBe(true);
  });

  it("повёрнутая набок занимает другое место", () => {
    const view = turned(fitted(wide, box), wide, box);
    // 1200×4000 при масштабе 0.15: столбик 180×600.
    expect(hits(view, wide, { x: 0, y: 290 })).toBe(true);
    expect(hits(view, wide, { x: 200, y: 0 })).toBe(false);
  });

  it("дрогнувшая рука — всё ещё щелчок, а потянувшая — уже нет", () => {
    const from = { x: 100, y: 100 };
    expect(isClick(from, { x: 102, y: 103 })).toBe(true);
    expect(isClick(from, { x: 110, y: 100 })).toBe(false);
  });
});
