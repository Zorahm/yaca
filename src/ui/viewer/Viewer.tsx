/**
 * Image viewer: one for the whole window.
 *
 * Opens from any tile: an attachment in the composer, a message card, a `ReadImage` row. There
 * is deliberately no separate viewer for "mine" and "the model's": it is the same action on the
 * same file, and two viewers would drift apart in small details at the first edit.
 *
 * Dialog behaviour belongs to Radix, as for the panels (§1.1): focus trap, `Esc`, roles (§14).
 * The zoom arithmetic is in `zoom.ts`; here there are only gestures.
 *
 * The picture comes from the core whole, as a `data:` address: 320-point thumbnails are too
 * small for looking closely, and the webview does not read the disk.
 */

import {
  ArrowClockwise,
  ArrowsIn,
  CaretLeft,
  CaretRight,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  X,
} from "@phosphor-icons/react";
import { Dialog } from "radix-ui";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  type ReactNode,
  type WheelEvent,
} from "react";

import { imageFull } from "../../core/client";
import { useBrowserCover } from "../browser/cover";
import { t } from "../../i18n";
import { plainPath } from "../format";
import { OpenImage, PreviewRoot } from "./context";
import {
  STEP,
  actual,
  clamp,
  fitted,
  hits,
  isClick,
  isFitted,
  panned,
  percent,
  turned,
  zoomAt,
  type Size,
  type View,
} from "./zoom";

/**
 * Gives the window the root for thumbnails and the viewer for everything drawn in it.
 *
 * `gallery` is the conversation's pictures in order. A picture opened from it is paged through
 * it; one opened not from it — an attachment not yet sent — is shown alone.
 */
export function Pictures({
  root,
  gallery,
  children,
}: {
  root: string | null;
  gallery: string[];
  children: ReactNode;
}) {
  const [at, setAt] = useState<{ list: string[]; index: number } | null>(null);
  // The viewer covers the window, and the browser's page with it.
  useBrowserCover(at !== null);

  const open = useCallback(
    (path: string) => {
      const index = gallery.indexOf(path);
      setAt(index === -1 ? { list: [path], index: 0 } : { list: gallery, index });
    },
    [gallery],
  );

  return (
    <PreviewRoot.Provider value={root}>
      <OpenImage.Provider value={open}>
        {children}
        {at !== null && root !== null && (
          <Viewer
            root={root}
            list={at.list}
            index={at.index}
            onIndex={(index) => setAt({ list: at.list, index })}
            onClose={() => setAt(null)}
          />
        )}
      </OpenImage.Provider>
    </PreviewRoot.Provider>
  );
}

type Loaded = { src: string; size: Size };

function Viewer({
  root,
  list,
  index,
  onIndex,
  onClose,
}: {
  root: string;
  list: string[];
  index: number;
  onIndex: (index: number) => void;
  onClose: () => void;
}) {
  const path = list[index];
  const [image, setImage] = useState<Loaded | "failed" | null>(null);
  const [box, setBox] = useState<Size | null>(null);
  const [view, setView] = useState<View | null>(null);
  // State, not `useRef`: Radix draws the dialog's content through a portal, one pass later,
  // and the node does not exist yet at the first effect. With `useRef` the frame was never
  // measured, and the picture, having nothing to fit into, never appeared at all.
  const [stage, setStage] = useState<HTMLDivElement | null>(null);
  const drag = useRef<{ x: number; y: number } | null>(null);
  const [dragging, setDragging] = useState(false);
  /** Where the press happened, to tell a click from a drag when it is released. */
  const pressed = useRef<{ x: number; y: number } | null>(null);
  // Whether the view was "fitted" is remembered before the frame changes: after the change there
  // is nothing left to ask — the "fitted" scale has become a different one.
  const wasFitted = useRef(true);

  useEffect(() => {
    let alive = true;
    setImage(null);
    setView(null);
    imageFull(root, path).then(
      (src) => {
        // The size is needed before the first frame: without it "fitted" cannot be computed.
        const probe = new Image();
        probe.onload = () => {
          if (alive) {
            setImage({ src, size: { width: probe.naturalWidth, height: probe.naturalHeight } });
          }
        };
        probe.onerror = () => alive && setImage("failed");
        probe.src = src;
      },
      () => alive && setImage("failed"),
    );
    return () => {
      alive = false;
    };
  }, [root, path]);

  useLayoutEffect(() => {
    if (stage === null) return;
    const measure = () => setBox({ width: stage.clientWidth, height: stage.clientHeight });
    measure();
    const watch = new ResizeObserver(measure);
    watch.observe(stage);
    return () => watch.disconnect();
  }, [stage]);

  const loaded = image !== null && image !== "failed" ? image : null;
  const size = loaded?.size ?? null;

  // The window was resized: a view that was "fitted" stays fitted, the rest stays within the new
  // frame. The first frame of a new picture also lands here.
  useEffect(() => {
    if (size === null || box === null) return;
    setView((now) => {
      if (now === null) return fitted(size, box);
      return wasFitted.current ? fitted(size, box, now.turns) : clamp(now, size, box);
    });
  }, [size, box]);

  useEffect(() => {
    wasFitted.current =
      view === null || size === null || box === null || isFitted(view, size, box);
  }, [view, size, box]);

  /** A gesture over the view; before loading there is nothing to change. */
  function change(next: (view: View, size: Size, box: Size) => View) {
    if (view === null || size === null || box === null) return;
    setView(next(view, size, box));
  }

  /** The event point relative to the frame's centre — `zoom` computes in these coordinates. */
  function fromCentre(event: { clientX: number; clientY: number }) {
    const rect = stage?.getBoundingClientRect();
    if (rect === undefined) return { x: 0, y: 0 };
    return {
      x: event.clientX - rect.left - rect.width / 2,
      y: event.clientY - rect.top - rect.height / 2,
    };
  }

  function onWheel(event: WheelEvent) {
    const at = fromCentre(event);
    // A mouse wheel steps in hundreds, a trackpad in units; the exponent smooths out both.
    const factor = Math.pow(STEP, -event.deltaY / 100);
    change((v, s, b) => zoomAt(v, factor, at, s, b));
  }

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: event.clientX, y: event.clientY };
    pressed.current = { x: event.clientX, y: event.clientY };
    setDragging(true);
  }

  function release() {
    drag.current = null;
    pressed.current = null;
    setDragging(false);
  }

  /**
   * Released: a click on empty space closes the viewer.
   *
   * On the picture it does not: the picture is dragged and double-clicked, and when it is
   * enlarged to fill the frame it leaves no empty space at all. While there is no picture — it
   * is loading or failed to open — the whole frame is empty space.
   */
  function onPointerUp(event: PointerEvent) {
    const from = pressed.current;
    release();
    if (from === null || !isClick(from, { x: event.clientX, y: event.clientY })) return;
    const onImage = view !== null && size !== null && hits(view, size, fromCentre(event));
    if (!onImage) onClose();
  }

  function onPointerMove(event: PointerEvent) {
    const from = drag.current;
    if (from === null) return;
    drag.current = { x: event.clientX, y: event.clientY };
    change((v, s, b) => panned(v, event.clientX - from.x, event.clientY - from.y, s, b));
  }

  const centre = { x: 0, y: 0 };
  const zoomIn = () => change((v, s, b) => zoomAt(v, STEP, centre, s, b));
  const zoomOut = () => change((v, s, b) => zoomAt(v, 1 / STEP, centre, s, b));
  const fit = () => change((v, s, b) => fitted(s, b, v.turns));
  const full = () => change(actual);
  const turn = () => change(turned);
  const many = list.length > 1;
  const step = (by: number) => onIndex((index + by + list.length) % list.length);

  /** The margins of the panels are the same empty space; there are no buttons or captions on them. */
  function closeOnBare(event: MouseEvent<HTMLElement>) {
    if (event.target === event.currentTarget) onClose();
  }

  function onKeyDown(event: KeyboardEvent) {
    switch (event.key) {
      case "ArrowLeft":
        if (many) step(-1);
        break;
      case "ArrowRight":
        if (many) step(1);
        break;
      case "+":
      case "=":
        zoomIn();
        break;
      case "-":
        zoomOut();
        break;
      case "0":
        fit();
        break;
      case "1":
        full();
        break;
      case "r":
      case "R":
      // The same key in the Russian keyboard layout.
      case "к":
      case "К":
        turn();
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  const fittedNow = view !== null && size !== null && box !== null && isFitted(view, size, box);

  return (
    <Dialog.Root open onOpenChange={(open) => !open && onClose()} modal={true}>
      <Dialog.Portal>
        <Dialog.Overlay className="viewer-overlay" />
        <Dialog.Content
          className="viewer"
          aria-describedby={undefined}
          onKeyDown={onKeyDown}
        >
          <header className="viewer-head" onClick={closeOnBare}>
            <Dialog.Title className="viewer-title">{plainPath(path)}</Dialog.Title>
            <Dialog.Close
              className="top-icon"
              aria-label={t("common.close")}
              title={t("common.close")}
            >
              <X size={15} />
            </Dialog.Close>
          </header>

          <div
            ref={setStage}
            className={dragging ? "viewer-stage viewer-dragging" : "viewer-stage"}
            onWheel={onWheel}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={release}
            // Double-click toggles back and forth between "fitted" and "1:1".
            onDoubleClick={(event) => {
              const at = fromCentre(event);
              change((v, s, b) =>
                isFitted(v, s, b) ? zoomAt(v, 1 / v.scale, at, s, b) : fitted(s, b, v.turns),
              );
            }}
          >
            {image === null && <span className="viewer-note">{t("viewer.loading")}</span>}
            {image === "failed" && <span className="viewer-note">{t("viewer.failed")}</span>}
            {loaded !== null && view !== null && (
              <img
                className="viewer-image"
                src={loaded.src}
                alt=""
                draggable={false}
                style={{
                  width: loaded.size.width,
                  height: loaded.size.height,
                  transform: `translate(-50%, -50%) translate(${view.x}px, ${view.y}px) scale(${view.scale}) rotate(${view.turns * 90}deg)`,
                }}
              />
            )}
          </div>

          <footer className="viewer-bar" onClick={closeOnBare}>
            {many && (
              <>
                <Tool label={t("viewer.prev")} onClick={() => step(-1)}>
                  <CaretLeft size={15} />
                </Tool>
                <span className="viewer-count">
                  {t("viewer.count", { at: index + 1, of: list.length })}
                </span>
                <Tool label={t("viewer.next")} onClick={() => step(1)}>
                  <CaretRight size={15} />
                </Tool>
                <span className="viewer-gap" />
              </>
            )}
            <Tool label={t("viewer.zoom_out")} onClick={zoomOut} disabled={view === null}>
              <MagnifyingGlassMinus size={15} />
            </Tool>
            <span className="viewer-count">{view === null ? "—" : `${percent(view)}%`}</span>
            <Tool label={t("viewer.zoom_in")} onClick={zoomIn} disabled={view === null}>
              <MagnifyingGlassPlus size={15} />
            </Tool>
            <span className="viewer-gap" />
            <Tool label={t("viewer.fit")} onClick={fit} disabled={view === null} pressed={fittedNow}>
              <ArrowsIn size={15} />
            </Tool>
            <Tool
              label={t("viewer.actual")}
              onClick={full}
              disabled={view === null}
              pressed={view !== null && view.scale === 1}
            >
              <span className="viewer-one">1:1</span>
            </Tool>
            <Tool label={t("viewer.rotate")} onClick={turn} disabled={view === null}>
              <ArrowClockwise size={15} />
            </Tool>
          </footer>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function Tool({
  label,
  onClick,
  disabled = false,
  pressed,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  pressed?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      className="top-icon"
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
