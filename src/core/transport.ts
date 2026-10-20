/**
 * The two roads to the host, behind one pair of functions.
 *
 * Locally the host is in the shell's process and Tauri carries the call; with a
 * server it is a socket. The names of the commands and the channels are the
 * same either way, so `client.ts` never asks which one it is on.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { isRemote } from "./link";
import { link } from "./remote";

/** A command of the host, by name. The arguments are the object `invoke` always took. */
export function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (isRemote()) return link.call<T>(cmd, args);
  return invoke<T>("host_call", { cmd, args });
}

/** A channel of the host's events. Resolves to the unsubscribe function. */
export function listenChannel<T>(channel: string, handler: (payload: T) => void): Promise<() => void> {
  if (isRemote()) return Promise.resolve(link.listen<T>(channel, handler));
  return listen<T>(channel, (event) => handler(event.payload));
}
