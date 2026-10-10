/**
 * Арифметика просмотрщика: масштаб, сдвиг и поворот картинки в рамке.
 *
 * Отдельно от компонента по той же причине, что и `feedScroll`: тесты идут в
 * node без раскладки, а «где окажется точка под курсором после колеса» —
 * вопрос к числам, а не к браузеру.
 *
 * Координаты сдвига — от центра рамки: `x = 0, y = 0` значит «картинка по
 * центру». Масштаб 1 — точка в точку.
 */

export type Size = { width: number; height: number };
export type Point = { x: number; y: number };

export type View = {
  scale: number;
  x: number;
  y: number;
  /** Четверти оборота по часовой стрелке. */
  turns: number;
};

/** Дальше восьми крат на экране уже квадраты, а не детали. */
export const MAX_SCALE = 8;

/** Шаг кнопок «ближе» и «дальше». */
export const STEP = 1.25;

/** Сколько места картинка занимает в рамке: повёрнутая набок — наоборот. */
export function shown(image: Size, turns: number): Size {
  return turns % 2 === 0 ? image : { width: image.height, height: image.width };
}

/**
 * Масштаб, при котором картинка видна целиком.
 *
 * Не больше единицы: маленькую иконку растягивать на всё окно незачем, она
 * от этого не станет подробнее.
 */
export function fitScale(image: Size, box: Size, turns: number): number {
  const { width, height } = shown(image, turns);
  if (width === 0 || height === 0) return 1;
  return Math.min(1, box.width / width, box.height / height);
}

/** Картинка целиком и по центру — с этого просмотр начинается. */
export function fitted(image: Size, box: Size, turns = 0): View {
  return { scale: fitScale(image, box, turns), x: 0, y: 0, turns };
}

export function isFitted(view: View, image: Size, box: Size): boolean {
  const fit = fitScale(image, box, view.turns);
  return Math.abs(view.scale - fit) < 1e-6 && view.x === 0 && view.y === 0;
}

/**
 * Возвращает вид в допустимые пределы.
 *
 * Мельче, чем «целиком», не бывает: дальше только поля. Сдвиг ограничен
 * краем картинки: пока она меньше рамки по оси, по этой оси она стоит по
 * центру, а когда больше — её край не уходит внутрь рамки.
 */
export function clamp(view: View, image: Size, box: Size): View {
  const fit = fitScale(image, box, view.turns);
  const scale = Math.min(Math.max(view.scale, fit), Math.max(MAX_SCALE, fit));
  const { width, height } = shown(image, view.turns);
  const reachX = Math.max(0, (width * scale - box.width) / 2);
  const reachY = Math.max(0, (height * scale - box.height) / 2);
  return {
    ...view,
    scale,
    x: Math.min(Math.max(view.x, -reachX), reachX),
    y: Math.min(Math.max(view.y, -reachY), reachY),
  };
}

/**
 * Масштаб вокруг точки: то, что было под курсором, под ним и остаётся.
 *
 * `at` — от центра рамки. Без этого колесо тянуло бы к центру картинки, и
 * чтобы рассмотреть угол, его пришлось бы сперва туда притащить.
 */
export function zoomAt(view: View, factor: number, at: Point, image: Size, box: Size): View {
  const wanted = clamp({ ...view, scale: view.scale * factor }, image, box).scale;
  const ratio = wanted / view.scale;
  return clamp(
    {
      ...view,
      scale: wanted,
      x: at.x - (at.x - view.x) * ratio,
      y: at.y - (at.y - view.y) * ratio,
    },
    image,
    box,
  );
}

export function panned(view: View, dx: number, dy: number, image: Size, box: Size): View {
  return clamp({ ...view, x: view.x + dx, y: view.y + dy }, image, box);
}

/** Точка в точку, вокруг центра рамки. */
export function actual(view: View, image: Size, box: Size): View {
  return zoomAt(view, 1 / view.scale, { x: 0, y: 0 }, image, box);
}

/**
 * Четверть оборота по часовой. Вид сбрасывается к «целиком»: после поворота
 * прежний сдвиг показывал бы уже другое место.
 */
export function turned(view: View, image: Size, box: Size): View {
  return fitted(image, box, (view.turns + 1) % 4);
}

/**
 * Попала ли точка в картинку. `at` — от центра рамки.
 *
 * По этому щелчок отличает картинку от пустого места вокруг неё: по пустому
 * просмотрщик закрывается. Когда картинка увеличена во всю рамку, пустого
 * места нет — и щелчок ничего не закрывает.
 */
export function hits(view: View, image: Size, at: Point): boolean {
  const { width, height } = shown(image, view.turns);
  return (
    Math.abs(at.x - view.x) <= (width * view.scale) / 2 &&
    Math.abs(at.y - view.y) <= (height * view.scale) / 2
  );
}

/**
 * Дальше этого от места нажатия — уже перетаскивание, а не щелчок. Рука на
 * кнопке мыши не стоит неподвижно, и без допуска щелчок по пустому месту
 * через раз считался бы попыткой сдвинуть картинку.
 */
export const CLICK_SLOP = 4;

export function isClick(from: Point, to: Point): boolean {
  return Math.hypot(to.x - from.x, to.y - from.y) <= CLICK_SLOP;
}

/** Масштаб в процентах для подписи. */
export function percent(view: View): number {
  return Math.round(view.scale * 100);
}
