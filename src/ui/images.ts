/**
 * Images in the composer: paste from the clipboard and drag into the window (§7.2).
 *
 * Kept apart from the screens, because there is one composer and two screens —
 * the session and the home screen — and both give it the same thing: the project
 * root that paths are counted from.
 */

import { hasFileDialogs, onFilesDragging, onFilesDropped, stashImage } from "../core/client";
import type { ComposerActions } from "./Composer";
import { relativeTo } from "./format";

type ImageActions = Pick<ComposerActions, "stashImage" | "onDropped" | "onDragging">;

/**
 * One set per root. The composer subscribes to dragging in an effect, and an
 * action that is new on every render would resubscribe it on every render.
 */
const made = new Map<string, ImageActions>();

/** Without a project or the shell there are no actions, and paste stays plain. */
export function imageActions(root: string | null): ImageActions {
  if (root === null || !hasFileDialogs) return {};
  let actions = made.get(root);
  if (actions === undefined) {
    actions = {
      stashImage: (data) => stashImage(root, data),
      // Paths are relative to the root, as with `＋`: the chip and the card in
      // §5.1 show what the model sees. A file outside the project stays absolute.
      onDropped: (listener) =>
        onFilesDropped((paths) => listener(paths.map((path) => relativeTo(root, path)))),
      onDragging: onFilesDragging,
    };
    made.set(root, actions);
  }
  return actions;
}
