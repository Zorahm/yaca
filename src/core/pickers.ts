/**
 * Choosing things on a disk the window cannot see.
 *
 * Against a server the native dialogs show this machine's disk, and the project
 * lives on the other one. The folder is chosen in a layer of the page instead,
 * listing the server's folders (`list_dir`), and this is the line between that
 * layer and whoever asked: the asker gets a promise, the layer gets a request.
 */

import type { DirListing } from "./bindings/DirListing";
import { call } from "./transport";

export type { DirListing };

type FolderRequest = { title: string; resolve: (path: string | null) => void };

let layer: ((request: FolderRequest) => void) | null = null;

/** The folder layer announces itself while it is mounted. Returns the way to leave. */
export function serveFolderRequests(handler: (request: FolderRequest) => void): () => void {
  layer = handler;
  return () => {
    if (layer === handler) layer = null;
  };
}

/** Asks the person for a folder of the server. `null` when they decline or nothing can ask. */
export function askServerFolder(title: string): Promise<string | null> {
  const ask = layer;
  if (ask === null) return Promise.resolve(null);
  return new Promise((resolve) => ask({ title, resolve }));
}

export function listDir(path: string | null, hidden: boolean): Promise<DirListing> {
  return call<DirListing>("list_dir", { path, hidden });
}

export function makeDir(parent: string, name: string): Promise<string> {
  return call<string>("make_dir", { parent, name });
}

/** The Markdown of a recorded session, to be saved on this side. */
export function exportText(project: string, id: string): Promise<string> {
  return call<string>("export_session_text", { project, id });
}

/** Hands text to the person as a file: the share sheet on a phone, a download elsewhere. */
export async function giveFile(name: string, text: string): Promise<void> {
  const file = new File([text], name, { type: "text/markdown" });
  if (typeof navigator.canShare === "function" && navigator.canShare({ files: [file] })) {
    await navigator.share({ files: [file], title: name });
    return;
  }
  const url = URL.createObjectURL(file);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  document.body.append(link);
  link.click();
  link.remove();
  // After the click has been handled; revoking at once can cancel the download.
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/** Pictures from the phone's gallery or camera, as base64 without the prefix. */
export function pickImages(): Promise<string[]> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.multiple = true;
    input.onchange = async () => {
      const files = Array.from(input.files ?? []);
      const read = (file: File) =>
        new Promise<string>((done, fail) => {
          const reader = new FileReader();
          reader.onload = () => done(String(reader.result).replace(/^data:[^,]*,/, ""));
          reader.onerror = () => fail(reader.error);
          reader.readAsDataURL(file);
        });
      resolve(await Promise.all(files.map(read)));
    };
    // Closing the picker without choosing fires `cancel`, not `change`.
    input.oncancel = () => resolve([]);
    input.click();
  });
}
