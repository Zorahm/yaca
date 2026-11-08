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
 * Full images use core data addresses or bounded delivery URLs: thumbnails are too
 * small for looking closely, and the webview has no general access to the disk.
 */

import {
  ArrowClockwise,
  ArrowsIn,
  CaretLeft,
  CaretRight,
  Copy,
  DotsThree,
  DownloadSimple,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  X,
} from "@phosphor-icons/react";
import { ContextMenu, Dialog, DropdownMenu } from "radix-ui";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from "react";

import { imageFull, saveImage } from "../../core/client";
import { sentImage } from "../../core/sentImage";
import { useBrowserCover } from "../browser/cover";
import { t } from "../../i18n";
import { plainPath } from "../format";
import { DeliveryRoot, OpenImage, PreviewRoot } from "./context";
import { copyImage } from "./imageActions";
import { useImageGestures } from "./useImageGestures";
import {
  STEP,
  actual,
  clamp,
  fitted,
  hits,
  isFitted,
  percent,
  turned,
  zoomAt,
  type Point,
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
  deliveryRoot = root,
  gallery,
  children,
}: {
  root: string | null;
  deliveryRoot?: string | null;
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
      <DeliveryRoot.Provider value={deliveryRoot}>
        <OpenImage.Provider value={open}>
          {children}
          {at !== null && root !== null && (
            <Viewer
              root={root}
              deliveryRoot={deliveryRoot ?? root}
              list={at.list}
              index={at.index}
              onIndex={(index) => setAt({ list: at.list, index })}
              onClose={() => setAt(null)}
            />
          )}
        </OpenImage.Provider>
      </DeliveryRoot.Provider>
    </PreviewRoot.Provider>
  );
}

type Loaded = { element: HTMLImageElement; size: Size };
type Action = "copy" | "save";
type Feedback = "viewer.copied" | "viewer.copy_failed" | "viewer.save_failed";

function Viewer({
  root,
  deliveryRoot,
  list,
  index,
  onIndex,
  onClose,
}: {
  root: string;
  deliveryRoot: string;
  list: string[];
  index: number;
  onIndex: (index: number) => void;
  onClose: () => void;
}) {
  const path = list[index];
  const imageRoot = sentImage(path) === null ? root : deliveryRoot;
  const [image, setImage] = useState<Loaded | "failed" | null>(null);
  const [box, setBox] = useState<Size | null>(null);
  const [view, setView] = useState<View | null>(null);
  // State, not `useRef`: Radix draws the dialog's content through a portal, one pass later,
  // and the node does not exist yet at the first effect. With `useRef` the frame was never
  // measured, and the picture, having nothing to fit into, never appeared at all.
  const [stage, setStage] = useState<HTMLDivElement | null>(null);
  const [contextOpen, setContextOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<{ path: string; message: Feedback } | null>(null);
  // Whether the view was "fitted" is remembered before the frame changes: after the change there
  // is nothing left to ask — the "fitted" scale has become a different one.
  const wasFitted = useRef(true);

  useLayoutEffect(() => {
    let alive = true;
    setImage(null);
    setView(null);
    imageFull(imageRoot, path).then(
      (src) => {
        // The size is needed before the first frame: without it "fitted" cannot be computed.
        const probe = new Image();
        probe.onload = () => {
          if (alive) {
            setImage({ element: probe, size: { width: probe.naturalWidth, height: probe.naturalHeight } });
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
  }, [imageRoot, path]);

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
    if (size === null || box === null || box.width === 0 || box.height === 0) return;
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
  const change = useCallback((next: (view: View, size: Size, box: Size) => View) => {
    if (size === null || box === null || box.width === 0 || box.height === 0) return;
    setView((current) => current === null ? null : next(current, size, box));
  }, [size, box]);

  /** The event point relative to the frame's centre — `zoom` computes in these coordinates. */
  const fromCentre = useCallback((event: { clientX: number; clientY: number }) => {
    const rect = stage?.getBoundingClientRect();
    if (rect === undefined) return { x: 0, y: 0 };
    return {
      x: event.clientX - rect.left - rect.width / 2,
      y: event.clientY - rect.top - rect.height / 2,
    };
  }, [stage]);

  useEffect(() => {
    if (stage === null) return;
    function onWheel(event: WheelEvent) {
      // Trackpad pinch is delivered as Ctrl+wheel and would also zoom the webview.
      event.preventDefault();
      const at = fromCentre(event);
      const factor = Math.pow(STEP, -event.deltaY / 100);
      change((v, s, b) => zoomAt(v, factor, at, s, b));
    }
    stage.addEventListener("wheel", onWheel, { passive: false });
    return () => stage.removeEventListener("wheel", onWheel);
  }, [stage, fromCentre, change]);

  function onImage(at: Point) {
    return view !== null && size !== null && hits(view, size, at);
  }

  function toggleZoom(at: Point, touch: boolean) {
    change((v, s, b) => {
      if (!isFitted(v, s, b)) return fitted(s, b, v.turns);
      return zoomAt(v, touch && v.scale === 1 ? 2 : 1 / v.scale, at, s, b);
    });
  }

  const gestures = useImageGestures(fromCentre, change, (at) => {
    if (onImage(at)) return true;
    onClose();
    return false;
  }, (at) => toggleZoom(at, true));

  const actionsDisabled = loaded === null || busy;
  async function runAction(action: Action) {
    if (loaded === null || busy) return;
    setBusy(true);
    setFeedback(null);
    try {
      if (action === "copy") {
        await copyImage(imageRoot, path, loaded.element);
        setFeedback({ path, message: "viewer.copied" });
      } else {
        await saveImage(imageRoot, path, t("viewer.save"));
      }
    } catch {
      setFeedback({ path, message: action === "copy" ? "viewer.copy_failed" : "viewer.save_failed" });
    } finally {
      setBusy(false);
    }
  }

  const centre = { x: 0, y: 0 };
  const zoomIn = () => change((v, s, b) => zoomAt(v, STEP, centre, s, b));
  const zoomOut = () => change((v, s, b) => zoomAt(v, 1 / STEP, centre, s, b));
  const fit = () => change((v, s, b) => fitted(s, b, v.turns));
  const full = () => change(actual);
  const turn = () => change(turned);
  const many = list.length > 1;
  const step = (by: number) => {
    gestures.reset();
    setFeedback(null);
    onIndex((index + by + list.length) % list.length);
  };

  /** The margins of the panels are the same empty space; there are no buttons or captions on them. */
  function closeOnBare(event: MouseEvent<HTMLElement>) {
    if (event.target === event.currentTarget) onClose();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.defaultPrevented || (event.target as HTMLElement).closest('[role="menu"]')) return;
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
            <Dialog.Title className="viewer-title">{sentImage(path)?.name ?? plainPath(path)}</Dialog.Title>
            <span className="viewer-feedback" role="status">
              {feedback?.path === path && t(feedback.message)}
            </span>
            <DropdownMenu.Root>
              <DropdownMenu.Trigger
                className="top-icon"
                aria-label={t("viewer.actions")}
                title={t("viewer.actions")}
                disabled={loaded === null}
              >
                <DotsThree size={18} weight="bold" />
              </DropdownMenu.Trigger>
              <DropdownMenu.Content className="pop viewer-menu" align="end" sideOffset={4}>
                <ImageActions Item={DropdownMenu.Item} disabled={actionsDisabled} onAction={runAction} />
              </DropdownMenu.Content>
            </DropdownMenu.Root>
            <Dialog.Close
              className="top-icon"
              aria-label={t("common.close")}
              title={t("common.close")}
            >
              <X size={15} />
            </Dialog.Close>
          </header>

          <ContextMenu.Root open={contextOpen} onOpenChange={(open) => {
            if (open && !gestures.canOpenMenu()) return;
            if (open) gestures.reset();
            setContextOpen(open);
          }}>
            <ContextMenu.Trigger asChild>
              <div
                ref={setStage}
                tabIndex={-1}
                className={gestures.dragging ? "viewer-stage viewer-dragging" : "viewer-stage"}
                {...gestures.handlers}
                onPointerDown={(event) => {
                  gestures.handlers.onPointerDown(event);
                  // Suppress Radix's long-press menu in the margins.
                  if (event.pointerType !== "mouse" && !onImage(fromCentre(event))) event.preventDefault();
                }}
                onContextMenu={(event) => {
                  if (!onImage(fromCentre(event))) event.preventDefault();
                }}
                onDoubleClick={(event) => {
                  if (!gestures.isTouch()) toggleZoom(fromCentre(event), false);
                }}
              >
                {image === null && <span className="viewer-note">{t("viewer.loading")}</span>}
                {image === "failed" && <span className="viewer-note">{t("viewer.failed")}</span>}
                {loaded !== null && view !== null && (
                  <img
                    className="viewer-image"
                    src={loaded.element.src}
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
            </ContextMenu.Trigger>
            <ContextMenu.Content className="pop viewer-menu" onCloseAutoFocus={(event) => {
              event.preventDefault();
              stage?.focus({ preventScroll: true });
            }}>
              <ImageActions Item={ContextMenu.Item} disabled={actionsDisabled} onAction={runAction} />
            </ContextMenu.Content>
          </ContextMenu.Root>

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

function ImageActions({ Item, disabled, onAction }: {
  Item: typeof ContextMenu.Item | typeof DropdownMenu.Item;
  disabled: boolean;
  onAction: (action: Action) => Promise<void>;
}) {
  return <>
    <Item className="pop-item" disabled={disabled} onSelect={() => void onAction("copy")}>
      <Copy size={14} />{t("viewer.copy")}
    </Item>
    <Item className="pop-item" disabled={disabled} onSelect={() => void onAction("save")}>
      <DownloadSimple size={14} />{t("viewer.save")}
    </Item>
  </>;
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
