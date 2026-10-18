/**
 * The chats' browsers as the window knows them: one state per chat, kept
 * whole as the shell sends it (`yaca://browser`), never pieced together here.
 */

import { useEffect, useRef, useState } from "react";

import { browser, type BrowserState, type SessionId } from "../../core/client";

export type Browsers = {
  /** This build has a browser at all. Until the shell answers, it has not. */
  supported: boolean;
  of: (session: SessionId | null) => BrowserState | null;
};

/**
 * `onFirstUse` is told when the model opens a chat's page for the first
 * time: the column comes up then, so that the human sees what the model
 * opened. Pages that were open before the window was (re)loaded are not news.
 */
export function useBrowsers(onFirstUse: (session: SessionId) => void): Browsers {
  const [supported, setSupported] = useState(false);
  const [states, setStates] = useState<ReadonlyMap<SessionId, BrowserState>>(new Map());
  const seen = useRef(new Set<SessionId>());
  const firstUse = useRef(onFirstUse);
  firstUse.current = onFirstUse;

  useEffect(() => {
    let live = true;
    void browser.supported().then((yes) => live && setSupported(yes));
    const off = browser.onState((state) => {
      if (!live) return;
      setStates((now) => new Map(now).set(state.session, state));
      if (state.opened && !seen.current.has(state.session)) {
        seen.current.add(state.session);
        if (state.busy) firstUse.current(state.session);
      }
    });
    void browser.states().then((list) => {
      if (!live) return;
      for (const state of list) if (state.opened) seen.current.add(state.session);
      // What arrived by event meanwhile is newer than this list.
      setStates((now) => {
        const next = new Map(now);
        for (const state of list) if (!next.has(state.session)) next.set(state.session, state);
        return next;
      });
    });
    return () => {
      live = false;
      void off.then((stop) => stop());
    };
  }, []);

  return {
    supported,
    of: (session) => (session === null ? null : (states.get(session) ?? null)),
  };
}
