/**
 * What lies at an attachment's path: a picture, a file, a folder, or nothing at all.
 *
 * Asked of the core, not guessed from the name: a `.png` that holds HTML is a
 * file, not a picture, and a folder without an extension cannot be told apart
 * by name at all. The webview does not read the disk, so the thumbnail comes
 * with the answer, as a ready `data:` address.
 */

import { useContext, useEffect, useState } from "react";

import { attachmentLook, type AttachmentLook } from "../core/client";
import { PreviewRoot } from "./viewer/context";

/** One question per path: the feed re-renders more often than files change. */
const asked = new Map<string, Promise<AttachmentLook>>();

/** The core did not answer, so only the name can be shown. */
const UNKNOWN: AttachmentLook = { kind: "file", preview: null };

function lookOf(root: string, path: string): Promise<AttachmentLook> {
  const key = `${root}\n${path}`;
  let answer = asked.get(key);
  if (answer === undefined) {
    answer = attachmentLook(root, path).catch(() => UNKNOWN);
    asked.set(key, answer);
  }
  return answer;
}

/** `undefined` — the answer is still on its way. */
export function useLook(path: string): AttachmentLook | undefined {
  const root = useContext(PreviewRoot);
  const [look, setLook] = useState<AttachmentLook | undefined>(undefined);

  useEffect(() => {
    if (root === null) return setLook(UNKNOWN);
    let alive = true;
    setLook(undefined);
    void lookOf(root, path).then((answer) => {
      if (alive) setLook(answer);
    });
    return () => {
      alive = false;
    };
  }, [root, path]);

  return look;
}
