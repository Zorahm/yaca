/**
 * The window's connection to a `yaca-server`.
 *
 * One WebSocket, JSON frames (the shape is `crates/yaca-server/src/wire.rs`):
 * a call goes out with a number and comes back with the same number, and the
 * host's events arrive on the channel names the desktop uses. Everything above
 * this file calls commands and listens to channels without knowing which
 * transport is under it.
 *
 * **A dropped line is survived, not repaired in place.** Chats live on the
 * server, so a reconnect loses nothing there; but this window's feed missed
 * whatever happened meanwhile, and the road that already catches a window up —
 * `attach_chats` at start — is the one that is tested. So the first reconnect
 * after a drop reloads the page and lets it take that road, instead of a second
 * way of catching up that would have to be kept in step with the first.
 */

import { socketAddress, type Remote } from "./link";

/** Why a connection is not usable. */
export type LinkFault = "unreachable" | "unauthorized" | "replaced" | "revoked" | "bad_address";

export class LinkError extends Error {
  constructor(readonly fault: LinkFault) {
    super(fault);
  }
}

export type LinkState =
  | { kind: "idle" }
  | { kind: "connecting" }
  | { kind: "ready" }
  /** The line dropped and a retry is under way. */
  | { kind: "lost" }
  /** Closed for a reason the person has to act on. */
  | { kind: "closed"; fault: LinkFault };

type Pending = { resolve: (value: unknown) => void; reject: (reason: unknown) => void };
type Handler = (payload: unknown) => void;

/** Clients ping this often; the server drops a socket quiet for three times as long. */
const PING_EVERY = 20_000;
/** Without any word from the server for this long, the line is taken for dead. */
const DEAD_AFTER = 50_000;
const BACKOFF = [500, 1_000, 2_000, 4_000, 8_000];

class Link {
  private socket: WebSocket | null = null;
  private remote: Remote | null = null;
  private next = 1;
  private pending = new Map<number, Pending>();
  private handlers = new Map<string, Set<Handler>>();
  private watchers = new Set<(state: LinkState) => void>();
  private ping: ReturnType<typeof setInterval> | null = null;
  private heard = 0;
  private retry: ReturnType<typeof setTimeout> | null = null;
  private attempts = 0;
  private wasReady = false;
  state: LinkState = { kind: "idle" };

  private set(state: LinkState): void {
    this.state = state;
    for (const watcher of this.watchers) watcher(state);
  }

  watch(watcher: (state: LinkState) => void): () => void {
    this.watchers.add(watcher);
    return () => this.watchers.delete(watcher);
  }

  /**
   * Connects and resolves once the server has accepted the token. Rejects with
   * a [`LinkError`] when the address is not one, the server cannot be reached,
   * or the token is refused. A server that was reached once keeps being retried
   * after a drop; one that has never answered is not.
   */
  connect(remote: Remote): Promise<void> {
    this.close();
    this.remote = remote;
    this.attempts = 0;
    this.wasReady = false;
    return this.open(true);
  }

  private open(first: boolean): Promise<void> {
    const remote = this.remote;
    const address = remote ? socketAddress(remote.url) : null;
    if (remote === null || address === null) {
      this.set({ kind: "closed", fault: "bad_address" });
      return Promise.reject(new LinkError("bad_address"));
    }
    this.set(first ? { kind: "connecting" } : { kind: "lost" });

    return new Promise<void>((resolve, reject) => {
      let settled = false;
      const finish = (error: LinkError | null) => {
        if (settled) return;
        settled = true;
        if (error === null) resolve();
        else reject(error);
      };

      let socket: WebSocket;
      try {
        socket = new WebSocket(address);
      } catch {
        this.set({ kind: "closed", fault: "unreachable" });
        finish(new LinkError("unreachable"));
        return;
      }
      this.socket = socket;

      socket.onopen = () => {
        socket.send(JSON.stringify({ t: "auth", token: remote.token, client: "yaca-ui" }));
      };
      socket.onmessage = (event) => {
        this.heard = Date.now();
        if (typeof event.data !== "string") return;
        const message = JSON.parse(event.data) as Record<string, unknown>;
        switch (message.t) {
          case "ready": {
            const reconnected = this.wasReady;
            this.wasReady = true;
            this.attempts = 0;
            this.startPing();
            this.set({ kind: "ready" });
            finish(null);
            // The feed missed what happened while the line was down: take the
            // road that catches a window up (see the header).
            if (reconnected) window.location.reload();
            break;
          }
          case "result": {
            const pending = this.pending.get(message.id as number);
            if (!pending) break;
            this.pending.delete(message.id as number);
            if (message.ok === true) pending.resolve(message.value);
            else pending.reject(String(message.error ?? "the server failed without a reason"));
            break;
          }
          case "event":
            this.handlers.get(String(message.channel))?.forEach((h) => h(message.payload));
            break;
          case "bye":
            this.bye(String(message.reason), finish);
            break;
          default:
            break;
        }
      };
      socket.onclose = () => {
        if (this.socket !== socket) return;
        this.socket = null;
        this.stopPing();
        this.failPending();
        // A refusal has already been recorded by `bye`.
        if (this.state.kind === "closed") return;
        if (!this.wasReady) {
          this.set({ kind: "closed", fault: "unreachable" });
          finish(new LinkError("unreachable"));
          return;
        }
        this.set({ kind: "lost" });
        this.schedule();
      };
      socket.onerror = () => {
        // `onclose` follows and does the work.
      };
    });
  }

  private bye(reason: string, finish: (error: LinkError | null) => void): void {
    const fault: LinkFault =
      reason === "replaced" ? "replaced" : reason === "revoked" ? "revoked" : "unauthorized";
    this.set({ kind: "closed", fault });
    finish(new LinkError(fault));
    this.close(false);
  }

  private schedule(): void {
    if (this.retry !== null) return;
    const wait = BACKOFF[Math.min(this.attempts, BACKOFF.length - 1)];
    this.attempts += 1;
    this.retry = setTimeout(() => {
      this.retry = null;
      void this.open(false).catch(() => {
        // `onclose` of the next socket schedules again; a refusal ends it.
      });
    }, wait);
  }

  private startPing(): void {
    this.stopPing();
    this.heard = Date.now();
    this.ping = setInterval(() => {
      if (Date.now() - this.heard > DEAD_AFTER) {
        // Silence is not an error event on a socket; a phone that left the
        // coverage never hears a close. Treat it as one.
        this.socket?.close();
        return;
      }
      if (this.socket?.readyState === WebSocket.OPEN) this.socket.send(JSON.stringify({ t: "ping" }));
    }, PING_EVERY);
  }

  private stopPing(): void {
    if (this.ping !== null) clearInterval(this.ping);
    this.ping = null;
  }

  private failPending(): void {
    for (const pending of this.pending.values()) pending.reject("the connection to the server was lost");
    this.pending.clear();
  }

  /** Closes for good. `reset` also forgets the state: a new `connect` follows. */
  close(reset = true): void {
    if (this.retry !== null) clearTimeout(this.retry);
    this.retry = null;
    this.stopPing();
    const socket = this.socket;
    this.socket = null;
    socket?.close();
    this.failPending();
    if (reset) this.set({ kind: "idle" });
  }

  call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    const socket = this.socket;
    if (this.state.kind !== "ready" || socket === null || socket.readyState !== WebSocket.OPEN) {
      return Promise.reject("not connected to the server");
    }
    const id = this.next++;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
      socket.send(JSON.stringify({ t: "call", id, cmd, args }));
    });
  }

  listen<T>(channel: string, handler: (payload: T) => void): () => void {
    let set = this.handlers.get(channel);
    if (!set) this.handlers.set(channel, (set = new Set()));
    set.add(handler as Handler);
    return () => set.delete(handler as Handler);
  }
}

/** The one connection of this window. */
export const link = new Link();
