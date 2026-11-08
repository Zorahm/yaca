/** The page is a native WebView2 positioned inside this themed panel. */
import {
  ArrowClockwise, ArrowLeft, ArrowRight, ArrowSquareOut, Camera, CaretDown, Code,
  GearSix, Globe, Minus, Plus, Square, WarningCircle, X, ChatText,
} from "@phosphor-icons/react";
import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";

import { browser, type BrowserState, type BrowserStep, type SessionId } from "../../core/client";
import { t, type Key } from "../../i18n";
import type { Bounds } from "../resize";
import { Splitter } from "../Splitter";
import { BrowserAddress } from "./BrowserAddress";
import { BrowserAccess } from "./BrowserAccess";
import { useCovered } from "./cover";

import "./browser.css";

const PRESETS = ["panel", "mobile", "tablet", "desktop"] as const;
const LOCAL = ["http://localhost:3000", "http://localhost:5173"];
const ZOOM = [25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300];
const ERRORS = {
  network: "browser.network",
  certificate: "browser.certificate",
  policy: "browser.blocked",
} as const satisfies Record<string, Key>;

export function BrowserColumn({
  session,
  state,
  project,
  width,
  bounds,
  initialWidth,
  onResize,
  onClose,
  onDocument,
  onChatContext,
}: {
  session: SessionId;
  state: BrowserState | null;
  project: string | null;
  width: number;
  bounds: Bounds | null;
  initialWidth: number;
  onResize: (width: number) => void;
  onClose: () => void;
  onDocument: (path: string) => void;
  onChatContext: (text: string) => void;
}) {
  const body = useRef<HTMLDivElement>(null);
  const panel = useRef<HTMLElement>(null);
  const covered = useCovered();
  const [problem, setProblem] = useState<Key | null>(null);
  const [options, setOptions] = useState(false);
  const [focus, setFocus] = useState(0);
  const [opening, setOpening] = useState(false);
  const tabs = state?.tabs ?? [];
  const currentTab = tabs.find((tab) => tab.id === state?.activeTab);
  const url = state?.url === "about:blank" ? "" : state?.url ?? "";
  const error = state?.error ?? null;
  const hasPage = !!url && error === null;
  const isWeb = /^https?:\/\//i.test(url);
  const busy = state?.busy ?? false;
  const loading = state?.loading ?? false;
  const disabled = busy || opening;
  const zoom = state?.zoom ?? 100;
  usePlacement(body, session, covered || !hasPage);

  const run = (work: Promise<unknown>) => {
    setProblem(null);
    return work.catch((err: unknown) => {
      const message = String(err);
      setProblem(message.includes("bad_address") ? "browser.bad_address" :
        message.includes("tab_limit") ? "browser.tab_limit" :
        message.includes("browser_busy") ? "browser.working" : "browser.action_failed");
    });
  };
  const open = (value: string) => {
    setOpening(true);
    void run(browser.open(session, value)).finally(() => setOpening(false));
  };
  const step = (which: BrowserStep) => void run(browser.go(session, which));
  const screenshot = () => void run(browser.screenshot(session, t("browser.screenshot")));
  const share = () => void run(browser.chatContext(session).then(onChatContext));
  const devtools = () => void run(browser.devtools(session));
  const external = () => void run(browser.external(session));
  const changeZoom = (direction: number) => {
    const next = direction === 0 ? 100 : direction > 0
      ? ZOOM.find((level) => level > zoom)
      : ZOOM.filter((level) => level < zoom).pop();
    if (next !== undefined) void run(browser.zoom(session, next));
  };
  const newTab = () => void run(browser.tab(session, "new"));
  const closeTab = () => void run(browser.tab(session, "close", state?.activeTab));

  useEffect(() => {
    const shortcut = (event: KeyboardEvent) => {
      if (covered || event.defaultPrevented || !panel.current?.contains(event.target as Node)) return;
      const key = event.key.toLowerCase();
      if (event.ctrlKey && !event.altKey && !event.shiftKey && key === "l" && !disabled) {
        event.preventDefault();
        setFocus((now) => now + 1);
      } else if (key === "f12" && hasPage) {
        event.preventDefault();
        devtools();
      } else if (event.ctrlKey && !event.altKey && !disabled) {
        if (event.shiftKey && key === "s" && hasPage) {
          event.preventDefault();
          screenshot();
        } else if (event.shiftKey && key === "enter" && hasPage) {
          event.preventDefault();
          share();
        } else if (event.shiftKey && key === "o" && hasPage && isWeb) {
          event.preventDefault();
          external();
        } else if (!event.shiftKey && key === "t") {
          event.preventDefault();
          newTab();
        } else if (!event.shiftKey && key === "w" && state !== null) {
          event.preventDefault();
          closeTab();
        } else if (hasPage && ["+", "=", "-", "0"].includes(key)) {
          event.preventDefault();
          changeZoom(key === "0" ? 0 : key === "-" ? -1 : 1);
        }
      }
    };
    document.addEventListener("keydown", shortcut);
    return () => document.removeEventListener("keydown", shortcut);
  });

  return (
    <aside
      ref={panel}
      className={bounds === null ? "browser-dock browser-dock-over" : "browser-dock"}
      style={{ width }}
      aria-label={t("browser.toggle")}
    >
      {bounds !== null && (
        <Splitter edge="left" width={width} bounds={bounds} initial={initialWidth}
          label={t("browser.resize")} onResize={onResize} />
      )}
      <div className="browser" data-agent={busy}>
        <header className="browser-tabs">
          <label className="browser-tab">
            {currentTab?.favicon ? <img src={currentTab.favicon} alt="" /> : <Globe size={16} />}
            <span>{currentTab?.title || currentTab?.url || t("browser.new_tab")}</span>
            <CaretDown size={12} />
            <select
              aria-label={t("browser.pages")}
              title={t("browser.pages")}
              disabled={disabled || tabs.length < 2}
              value={state?.activeTab ?? 0}
              onChange={(event) => void run(browser.tab(session, "select", Number(event.target.value)))}
            >
              {tabs.length === 0 && <option value={0}>{t("browser.new_tab")}</option>}
              {tabs.map((tab, index) => (
                <option key={tab.id} value={tab.id}>
                  {index + 1}. {tab.title || tab.url || t("browser.new_tab")}
                </option>
              ))}
            </select>
          </label>
          <IconButton label={t("browser.new_tab")} shortcut="Ctrl+T" disabled={disabled} onClick={newTab}>
            <Plus size={16} />
          </IconButton>
          <button className="browser-close" type="button" onClick={onClose}>
            <X size={14} />
            {t("browser.close")}
          </button>
        </header>

        <div className="browser-head">
          <IconButton label={t("browser.back")} disabled={disabled || !state?.canGoBack} onClick={() => step("back")}>
            <ArrowLeft size={16} />
          </IconButton>
          <IconButton label={t("browser.forward")} disabled={disabled || !state?.canGoForward} onClick={() => step("forward")}>
            <ArrowRight size={16} />
          </IconButton>
          {loading ? (
            <IconButton label={t("browser.stop")} onClick={() => step("stop")}>
              <Square size={13} weight="fill" />
            </IconButton>
          ) : (
            <IconButton label={t("browser.reload")} disabled={disabled || !url}
              onClick={() => error ? open(url) : step("reload")}>
              <ArrowClockwise size={16} />
            </IconButton>
          )}
          <BrowserAddress key={state?.activeTab} url={url} focus={focus} disabled={disabled} onOpen={open} />
          {loading && <span className="browser-progress" aria-label={t("browser.loading")} role="progressbar" />}
        </div>

        <BrowserAccess session={session} state={state} options={options} run={run} />
        {options && (
          <div className="browser-options">
            <button className="btn" type="button" disabled={disabled || state === null} onClick={closeTab}>
              {t("browser.close_tab")}
            </button>
            <div className="browser-zoom">
              <IconButton label={t("browser.zoom_out")} shortcut="Ctrl+−" disabled={disabled || !hasPage} onClick={() => changeZoom(-1)}>
                <Minus size={14} />
              </IconButton>
              <button type="button" className="btn" title={t("browser.zoom_reset")} disabled={disabled || !hasPage} onClick={() => changeZoom(0)}>
                {zoom}%
              </button>
              <IconButton label={t("browser.zoom_in")} shortcut="Ctrl++" disabled={disabled || !hasPage} onClick={() => changeZoom(1)}>
                <Plus size={14} />
              </IconButton>
            </div>
          </div>
        )}
        {problem !== null && <div className="browser-problem" role="alert">{t(problem)}</div>}
        <div className="browser-body" ref={body}>
          {error !== null ? (
            <div className="browser-empty" role="alert">
              <WarningCircle size={28} />
              <strong>{t(ERRORS[error])}</strong>
              <span className="browser-error-url">{url}</span>
              <button className="btn" type="button" disabled={disabled} onClick={() => open(url)}>
                {t("browser.retry")}
              </button>
            </div>
          ) : !hasPage && (
            <BrowserHome project={project} recent={state?.recent ?? []} loading={loading || opening}
              disabled={disabled} onOpen={open} onDocument={onDocument} />
          )}
        </div>

        <footer className="browser-foot">
          {state !== null && (
            <label className="browser-viewport">
              <span>{Math.round(state.width)}×{Math.round(state.height)}</span>
              <CaretDown size={11} />
              <select
                aria-label={t("browser.viewport")}
                title={t("browser.viewport")}
                disabled={disabled}
                value=""
                onChange={(event) => void run(browser.viewport(session, event.target.value))}
              >
                <option value="" disabled>{t("browser.viewport")}</option>
                {PRESETS.map((name) => <option key={name} value={name}>{t(`browser.preset_${name}`)}</option>)}
              </select>
            </label>
          )}
          {zoom !== 100 && <button type="button" className="browser-zoom-indicator" title={t("browser.zoom_reset")}
            disabled={disabled || !hasPage} onClick={() => changeZoom(0)}>{zoom}%</button>}
          <div className="browser-tools">
            <IconButton label={t("browser.screenshot")} shortcut="Ctrl+Shift+S" disabled={disabled || !hasPage} onClick={screenshot}>
              <Camera size={16} />
            </IconButton>
            <IconButton label={t("browser.share")} shortcut="Ctrl+Shift+Enter" disabled={disabled || !hasPage} onClick={share}>
              <ChatText size={16} />
            </IconButton>
            <IconButton label={t("browser.external")} shortcut="Ctrl+Shift+O" disabled={!hasPage || !isWeb} onClick={external}>
              <ArrowSquareOut size={16} />
            </IconButton>
            <IconButton label={t("browser.devtools")} shortcut="F12" disabled={!hasPage} onClick={devtools}>
              <Code size={17} />
              {!!state?.consoleErrors && <span className="browser-errors"
                aria-label={t("browser.console_errors", { count: state.consoleErrors })}>
                {Math.min(state.consoleErrors, 99)}{state.consoleErrors > 99 ? "+" : ""}
              </span>}
            </IconButton>
            <IconButton label={t("browser.settings")} expanded={options} onClick={() => setOptions(!options)}>
              <GearSix size={16} />
            </IconButton>
          </div>
        </footer>
      </div>
    </aside>
  );
}

function BrowserHome({ project, recent, loading, disabled, onOpen, onDocument }: {
  project: string | null;
  recent: string[];
  loading: boolean;
  disabled: boolean;
  onOpen: (url: string) => void;
  onDocument: (path: string) => void;
}) {
  const [docs, setDocs] = useState<string[]>([]);
  useEffect(() => {
    if (project === null) return;
    let active = true;
    void browser.projectDocs(project).then((paths) => {
      if (active) setDocs(paths);
    }, () => {
      if (active) setDocs([]);
    });
    return () => { active = false; };
  }, [project]);

  if (loading) {
    return <div className="browser-empty" role="status">{t("browser.loading")}</div>;
  }
  return (
    <div className="browser-empty browser-home">
      <Globe size={28} />
      <strong>{t("browser.new_tab")}</strong>
      <span>{t("browser.empty_body")}</span>
      {recent.length > 0 && (
        <section>
          <h3>{t("browser.recent")}</h3>
          {recent.map((url) => (
            <button className="browser-link" type="button" key={url} title={url}
              disabled={disabled} onClick={() => onOpen(url)}>{url}</button>
          ))}
        </section>
      )}
      <section>
        <h3>{t("browser.local_servers")}</h3>
        {LOCAL.map((url) => (
          <button className="browser-link" type="button" key={url} disabled={disabled}
            onClick={() => onOpen(url)}>{url.replace("http://", "")}</button>
        ))}
      </section>
      {docs.length > 0 && (
        <section>
          <h3>{t("browser.project_docs")}</h3>
          {docs.map((path) => (
            <button className="browser-link" type="button" key={path} onClick={() => onDocument(path)}>{path}</button>
          ))}
        </section>
      )}
    </div>
  );
}

export function IconButton({ label, shortcut, disabled, expanded, onClick, children }: {
  label: string;
  shortcut?: string;
  disabled?: boolean;
  expanded?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button type="button" className="browser-icon" aria-label={label} aria-expanded={expanded}
      title={shortcut ? label + " (" + shortcut + ")" : label} disabled={disabled} onClick={onClick}>
      {children}
    </button>
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
