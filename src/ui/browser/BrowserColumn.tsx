/**
 * The browser column: the chat's page, and a header for the human.
 *
 * The page is not drawn here. It is a native webview the shell lays over the
 * column's body, so what this component owns is the frame around it: where
 * the body is, measured every frame and sent to the shell when it moves;
 * the header with the arrows and the address; the empty state before the
 * page exists. Native means on top of everything — so the header has no
 * menus to unfold over the page, only buttons.
 *
 * The human and the model drive the same page. There is no mode for either:
 * the running line under the header says the model is acting, and that is
 * all the column does about it.
 */

import {
  ArrowClockwise,
  ArrowLeft,
  ArrowRight,
  ArrowSquareOut,
  Code,
  X,
} from "@phosphor-icons/react";
import { useEffect, useRef, useState, type FormEvent, type ReactNode, type RefObject } from "react";

import { browser, type BrowserState, type BrowserStep, type SessionId } from "../../core/client";
import { t } from "../../i18n";
import type { Bounds } from "../resize";
import { Splitter } from "../Splitter";
import { useCovered } from "./cover";

import "./browser.css";

export function BrowserColumn({
  session,
  state,
  width,
  bounds,
  initialWidth,
  onResize,
  onClose,
}: {
  session: SessionId;
  state: BrowserState | null;
  width: number;
  /** `null`: no room beside the feed, and the column lies over it. */
  bounds: Bounds | null;
  initialWidth: number;
  onResize: (width: number) => void;
  onClose: () => void;
}) {
  const body = useRef<HTMLDivElement>(null);
  const covered = useCovered();
  const [problem, setProblem] = useState<string | null>(null);
  usePlacement(body, session, covered);

  const opened = state?.opened ?? false;
  const step = (which: BrowserStep) => {
    setProblem(null);
    browser.go(session, which).catch((e: unknown) => console.error("browser step failed", e));
  };

  return (
    <aside
      className={bounds === null ? "browser-dock browser-dock-over" : "browser-dock"}
      style={{ width }}
      aria-label={t("browser.toggle")}
    >
      {bounds !== null && (
        <Splitter
          edge="left"
          width={width}
          bounds={bounds}
          initial={initialWidth}
          label={t("browser.resize")}
          onResize={onResize}
        />
      )}
      <div className="browser">
        <header className="browser-head">
          <IconButton
            label={t("browser.back")}
            disabled={!state?.canGoBack}
            onClick={() => step("back")}
          >
            <ArrowLeft size={15} />
          </IconButton>
          <IconButton
            label={t("browser.forward")}
            disabled={!state?.canGoForward}
            onClick={() => step("forward")}
          >
            <ArrowRight size={15} />
          </IconButton>
          {state?.loading ? (
            <IconButton label={t("browser.stop")} onClick={() => step("stop")}>
              <X size={15} />
            </IconButton>
          ) : (
            <IconButton label={t("browser.reload")} disabled={!opened} onClick={() => step("reload")}>
              <ArrowClockwise size={15} />
            </IconButton>
          )}
          <Address
            session={session}
            state={state}
            onProblem={setProblem}
          />
          {opened && (
            <>
              <IconButton
                label={t("browser.external")}
                onClick={() =>
                  browser.external(session).catch((e: unknown) => console.error("browser external failed", e))
                }
              >
                <ArrowSquareOut size={15} />
              </IconButton>
              <IconButton
                label={t("browser.devtools")}
                onClick={() =>
                  browser.devtools(session).catch((e: unknown) => console.error("browser devtools failed", e))
                }
              >
                <Code size={15} />
              </IconButton>
            </>
          )}
          <IconButton label={t("common.close")} onClick={onClose}>
            <X size={14} />
          </IconButton>
          {state?.busy && <span className="browser-line" aria-label={t("browser.busy")} role="status" />}
        </header>
        {problem !== null && <div className="browser-problem">{problem}</div>}
        <div className="browser-body" ref={body}>
          {!opened && (
            <div className="browser-empty">
              <strong>{t("browser.empty_title")}</strong>
              <span>{t("browser.empty_body")}</span>
            </div>
          )}
        </div>
      </div>
    </aside>
  );
}

function IconButton({
  label,
  disabled = false,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button className="top-icon" aria-label={label} title={label} disabled={disabled} onClick={onClick}>
      {children}
    </button>
  );
}

/**
 * The address and the size the page is laid out at. At rest the address is
 * shown with its host dimmed; a click turns it into a field.
 */
function Address({
  session,
  state,
  onProblem,
}: {
  session: SessionId;
  state: BrowserState | null;
  onProblem: (text: string | null) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const field = useRef<HTMLInputElement>(null);
  const url = state?.url && state.url !== "about:blank" ? state.url : "";

  useEffect(() => {
    // Selected once, when editing begins, not on every keystroke.
    if (draft !== null) field.current?.select();
  }, [draft !== null]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const text = (draft ?? "").trim();
    setDraft(null);
    if (text === "") return;
    onProblem(null);
    browser.open(session, text).catch((e: unknown) => {
      onProblem(t(String(e).includes("bad_address") ? "browser.bad_address" : "browser.open_failed"));
    });
  };

  const size =
    state === null ? null : `${Math.round(state.width)}×${Math.round(state.height)}`;

  return (
    <form className="browser-address" onSubmit={submit}>
      {draft === null ? (
        <button
          type="button"
          className="browser-url"
          title={url || t("browser.address")}
          onClick={() => setDraft(url)}
        >
          {url ? <Url url={url} /> : <span className="browser-host">{t("browser.address")}</span>}
        </button>
      ) : (
        <input
          ref={field}
          className="browser-input"
          autoFocus
          spellCheck={false}
          aria-label={t("browser.address")}
          placeholder={t("browser.address")}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => setDraft(null)}
          onKeyDown={(e) => {
            // The window's own `Esc` stops the agent; this one leaves the field.
            if (e.key === "Escape") {
              e.stopPropagation();
              setDraft(null);
            }
          }}
        />
      )}
      {size !== null && state?.source === "model" ? (
        <span className="browser-size browser-size-model">
          {t("browser.size_model", { size })}
          <button
            type="button"
            className="browser-size-reset"
            aria-label={t("browser.reset_size")}
            title={t("browser.reset_size")}
            onClick={() =>
              browser.resetViewport(session).catch((e: unknown) => console.error("browser reset failed", e))
            }
          >
            <X size={11} />
          </button>
        </span>
      ) : (
        size !== null && <span className="browser-size">{size}</span>
      )}
    </form>
  );
}

/** The address with its scheme and host dimmed: the path is what changes. */
function Url({ url }: { url: string }) {
  const match = /^(https?:\/\/[^/?#]+)(.*)$/.exec(url);
  if (match === null) return <span>{url}</span>;
  return (
    <>
      <span className="browser-host">{match[1].replace(/^https?:\/\//, "")}</span>
      <span>{match[2]}</span>
    </>
  );
}

/**
 * Tells the shell where the body of the column is, whenever it moves.
 *
 * Read every frame rather than observed: the column moves without changing
 * size — the panel beside it opens, a splitter is dragged — and nothing
 * fires for that. One rectangle a frame is nothing to read, and the shell is
 * told only when it changed.
 */
function usePlacement(body: RefObject<HTMLDivElement | null>, session: SessionId, covered: boolean) {
  useEffect(() => {
    let frame = 0;
    let last = "";
    const tick = () => {
      const element = body.current;
      if (element !== null) {
        const r = element.getBoundingClientRect();
        const key = covered ? "covered" : `${r.left}:${r.top}:${r.width}:${r.height}`;
        if (key !== last) {
          last = key;
          const rect = covered ? null : { x: r.left, y: r.top, width: r.width, height: r.height };
          browser
            .place(rect === null ? null : session, rect)
            .catch((e: unknown) => console.error("browser placement failed", e));
        }
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [body, session, covered]);

  // The column is gone: every page goes out of sight.
  useEffect(
    () => () => {
      browser.place(null, null).catch((e: unknown) => console.error("browser placement failed", e));
    },
    [],
  );
}
