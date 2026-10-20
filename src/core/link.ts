/**
 * Where the commands go: this machine's own host, or a `yaca-server`.
 *
 * On a desktop the host runs inside the shell and the window talks to it
 * through Tauri; the person may instead point the window at a server and work
 * on that machine's projects. On a phone there is no host in the app at all, so
 * a server is the only choice. The choice and the server's address live in the
 * webview's storage: they are about this device, not about any project, and
 * the shell's settings file belongs to a host that may not be the one in use.
 *
 * The token is kept there too, in the app's private storage. That is the same
 * protection a browser gives a saved login, which is the bar for a device the
 * person already carries; the server can revoke it at any time.
 */

import { inTauri } from "./demo";

const MODE_KEY = "yaca.link.mode";
const REMOTE_KEY = "yaca.link.remote";

export type LinkMode = "local" | "remote";

/** A server the window has been pointed at. */
export type Remote = {
  /** The `ws://` or `wss://` address of the socket. */
  url: string;
  token: string;
  /** What the person typed, to show back to them. */
  label: string;
};

/** A phone or a tablet: where the app has no host of its own. */
export const isMobile =
  inTauri && typeof navigator !== "undefined" && /Android|iPhone|iPad/i.test(navigator.userAgent);

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string | null): void {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Storage can be off (a private window). The choice then lasts until the
    // page is closed, which is what the caller gets from its own variable.
  }
}

export function savedRemote(): Remote | null {
  const text = read(REMOTE_KEY);
  if (text === null) return null;
  try {
    const parsed = JSON.parse(text) as Partial<Remote>;
    if (typeof parsed.url === "string" && typeof parsed.token === "string") {
      return { url: parsed.url, token: parsed.token, label: parsed.label ?? parsed.url };
    }
  } catch {
    // A damaged entry is no entry.
  }
  return null;
}

export function saveRemote(remote: Remote): void {
  write(REMOTE_KEY, JSON.stringify(remote));
}

export function forgetRemote(): void {
  write(REMOTE_KEY, null);
}

/**
 * The mode in force. A phone is always remote; a desktop is local unless the
 * person chose a server. Outside Tauri (the stand in a browser) there is
 * neither: the demo plays instead.
 */
export function linkMode(): LinkMode {
  if (isMobile) return "remote";
  return read(MODE_KEY) === "remote" ? "remote" : "local";
}

export function setLinkMode(mode: LinkMode): void {
  write(MODE_KEY, mode === "remote" ? "remote" : null);
}

/** Whether this window talks to a server. */
export const isRemote = (): boolean => inTauri && linkMode() === "remote";

/**
 * Turns what a person types into the socket's address.
 *
 * `yaca.example.com`, `https://yaca.example.com/` and `wss://yaca.example.com/ws`
 * all mean the same server. A bare host gets `wss`: a server on the open
 * internet is behind TLS, and a plain `ws://` has to be asked for in full.
 * Returns `null` for what cannot be an address.
 */
export function socketAddress(typed: string): string | null {
  let text = typed.trim();
  if (text === "") return null;
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(text)) text = `wss://${text}`;
  let url: URL;
  try {
    url = new URL(text);
  } catch {
    return null;
  }
  const secure = url.protocol === "https:" || url.protocol === "wss:";
  const plain = url.protocol === "http:" || url.protocol === "ws:";
  if (!secure && !plain) return null;
  if (url.hostname === "") return null;
  const path = url.pathname.replace(/\/+$/, "");
  return `${secure ? "wss" : "ws"}://${url.host}${path.endsWith("/ws") ? path : `${path}/ws`}`;
}

/**
 * The one-line link the install script prints: `yaca://connect?host=…&token=…`.
 * Pasting it into the address field fills both fields, which on a phone beats
 * typing a 68-character token. `plain=1` marks a server without TLS.
 */
export function parseConnectionLink(text: string): { address: string; token: string } | null {
  let url: URL;
  try {
    url = new URL(text.trim());
  } catch {
    return null;
  }
  if (url.protocol !== "yaca:") return null;
  const host = url.searchParams.get("host");
  const token = url.searchParams.get("token");
  if (host === null || host === "" || token === null || token === "") return null;
  return { address: `${url.searchParams.get("plain") === "1" ? "http" : "https"}://${host}`, token };
}
