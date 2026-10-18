/**
 * The browser column as the window holds it: whether it is open, how wide,
 * and how it shares the room right of the feed with the panel.
 *
 * One state for the window, not one per chat: the column is a column of the
 * window, and switching chats shows the other chat's page in it.
 *
 * Where the window is too narrow for both beside the feed, one of them is
 * open at a time: the one opened last stays, and the other closes. Where it
 * is too narrow even for the browser alone, the browser lies over the feed.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import type { BrowserState, SessionId } from "../../core/client";
import {
  BROWSER,
  BROWSER_SHARE,
  bothFit,
  browserBounds,
  clamp,
  PANEL,
  useStoredWidth,
  type Bounds,
} from "../resize";
import { useBrowsers } from "./useBrowsers";

export type BrowserColumnState = {
  supported: boolean;
  open: boolean;
  state: BrowserState | null;
  width: number;
  bounds: Bounds | null;
  initialWidth: number;
  setWidth: (width: number) => void;
  toggle: () => void;
  close: () => void;
  /** What the panel has to leave for the browser beside it. */
  reserve: number;
};

export function useBrowserColumn({
  session,
  windowWidth,
  sideWidth,
  panelOpen,
  closePanel,
}: {
  session: SessionId | null;
  windowWidth: number;
  sideWidth: number;
  panelOpen: boolean;
  closePanel: () => void;
}): BrowserColumnState {
  const [wanted, setWanted] = useState(false);
  const current = useRef(session);
  current.current = session;
  // The model opened its first page in the chat being looked at: the human
  // is shown what it opened.
  const browsers = useBrowsers((opened) => {
    if (opened === current.current) setWanted(true);
  });
  const [chosen, setChosen] = useStoredWidth("yaca.width.browser");

  const open = wanted && browsers.supported && session !== null;
  const both = bothFit(windowWidth, sideWidth);

  const before = useRef({ open, panelOpen });
  useEffect(() => {
    const was = before.current;
    before.current = { open, panelOpen };
    if (!open || !panelOpen || both) return;
    // The panel was just opened: the browser makes way. Otherwise the
    // browser was, or the window narrowed, and the panel does.
    if (panelOpen && !was.panelOpen) setWanted(false);
    else closePanel();
  }, [open, panelOpen, both, closePanel]);

  const bounds = browserBounds(windowWidth, sideWidth, panelOpen && both ? PANEL.min : 0);
  const initialWidth = Math.round(windowWidth * BROWSER_SHARE);
  const width =
    bounds === null
      ? Math.max(BROWSER.min, Math.min(chosen ?? initialWidth, windowWidth - sideWidth))
      : clamp(chosen ?? initialWidth, bounds);

  return {
    supported: browsers.supported,
    open,
    state: browsers.of(session),
    width,
    bounds,
    initialWidth,
    setWidth: setChosen,
    toggle: useCallback(() => setWanted((now) => !now), []),
    close: useCallback(() => setWanted(false), []),
    reserve: open && bounds !== null ? width : 0,
  };
}
