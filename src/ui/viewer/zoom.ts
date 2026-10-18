/**
 * The viewer's arithmetic: scale, pan and rotation of a picture in its frame.
 *
 * Kept apart from the component for the same reason as `feedScroll`: the tests run in node
 * without layout, and "where the point under the cursor ends up after the wheel" is a question
 * for numbers, not for the browser.
 *
 * Pan coordinates are relative to the frame's centre: `x = 0, y = 0` means "picture centred".
 * Scale 1 is 1:1.
 */

export type Size = { width: number; height: number };
export type Point = { x: number; y: number };

export type View = {
  scale: number;
  x: number;
  y: number;
  /** Quarter turns clockwise. */
  turns: number;
};

/** Beyond eight times the screen shows squares, not details. */
export const MAX_SCALE = 8;

/** The step of the zoom-in and zoom-out buttons. */
export const STEP = 1.25;

/** How much room the picture takes in the frame: turned on its side, the other way round. */
export function shown(image: Size, turns: number): Size {
  return turns % 2 === 0 ? image : { width: image.height, height: image.width };
}

/**
 * The scale at which the picture is seen whole.
 *
 * Never more than one: there is no point stretching a small icon to the whole window, it does
 * not get more detailed for it.
 */
export function fitScale(image: Size, box: Size, turns: number): number {
  const { width, height } = shown(image, turns);
  if (width === 0 || height === 0) return 1;
  return Math.min(1, box.width / width, box.height / height);
}

/** The picture whole and centred — viewing starts from here. */
export function fitted(image: Size, box: Size, turns = 0): View {
  return { scale: fitScale(image, box, turns), x: 0, y: 0, turns };
}

export function isFitted(view: View, image: Size, box: Size): boolean {
  const fit = fitScale(image, box, view.turns);
  return Math.abs(view.scale - fit) < 1e-6 && view.x === 0 && view.y === 0;
}

/**
 * Brings the view back within the allowed limits.
 *
 * Nothing smaller than fitted exists: beyond it there are only margins. The pan is bounded by
 * the picture's edge: while the picture is smaller than the frame on an axis, it stays centred
 * on that axis, and once larger, its edge does not move inside the frame.
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
 * Scale around a point: what was under the cursor stays under it.
 *
 * `at` is relative to the frame's centre. Without this the wheel would pull toward the picture's
 * centre, and to look at a corner you would first have to drag it there.
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

/** 1:1, around the centre of the frame. */
export function actual(view: View, image: Size, box: Size): View {
  return zoomAt(view, 1 / view.scale, { x: 0, y: 0 }, image, box);
}

/**
 * A quarter turn clockwise. The view resets to fitted: after a turn the old pan would show a
 * different place.
 */
export function turned(view: View, image: Size, box: Size): View {
  return fitted(image, box, (view.turns + 1) % 4);
}

/**
 * Whether the point falls on the picture. `at` is relative to the frame's centre.
 *
 * A click uses this to tell the picture from the empty space around it: on empty space the
 * viewer closes. When the picture is enlarged to fill the frame there is no empty space — and a
 * click closes nothing.
 */
export function hits(view: View, image: Size, at: Point): boolean {
  const { width, height } = shown(image, view.turns);
  return (
    Math.abs(at.x - view.x) <= (width * view.scale) / 2 &&
    Math.abs(at.y - view.y) <= (height * view.scale) / 2
  );
}

/**
 * Beyond this distance from the press point it is a drag, not a click. A hand on the mouse
 * button does not stay still, and without this tolerance a click on empty space would often
 * count as an attempt to pan the picture.
 */
export const CLICK_SLOP = 4;

export function isClick(from: Point, to: Point): boolean {
  return Math.hypot(to.x - from.x, to.y - from.y) <= CLICK_SLOP;
}

/** The scale in percent, for the label. */
export function percent(view: View): number {
  return Math.round(view.scale * 100);
}
