/**
 * All the pictures of the conversation in order — what the viewer pages through.
 *
 * Both the user's and the model's: what the person attached, what the agent opened via
 * `ReadImage`, and pictures a tool returned (a browser screenshot). For paging there is no
 * difference — it is one strip, and a picture the agent looked at in answer to a screenshot
 * stands right after the screenshot.
 */

import { flatRows, type Feed } from "../../core/feed";

const IMAGE = /\.(png|jpe?g|gif|webp|bmp)$/i;

/**
 * Whether a path looks like a picture — by its extension.
 *
 * Only here, where the core cannot be asked: the paging list is built from the feed by a pure
 * function. What a file really is, the bytes decide, and the tile asks the core about that
 * (`look`).
 */
export function isImagePath(path: string): boolean {
  return IMAGE.test(path);
}

export function galleryOf(feed: Feed | null): string[] {
  if (feed === null) return [];
  const out: string[] = [];
  const add = (path: string) => {
    // One path, one stop: a screenshot attached twice is the same file.
    if (isImagePath(path) && !out.includes(path)) out.push(path);
  };

  for (const block of feed.blocks) {
    if (block.kind === "human") {
      block.attachments.forEach(add);
    } else if (block.kind === "actions") {
      for (const row of flatRows(block.rows)) {
        // Only what opened: a failed call has nothing to show.
        if (row.state !== "done") continue;
        if (row.tool === "ReadImage" && row.target) add(row.target);
        // Images a tool returned, such as a page's screenshot, in the order
        // the tool gave them.
        row.meta?.pictures?.forEach(add);
      }
    }
  }
  return out;
}
