/**
 * What is needed to show a picture from anywhere in the window.
 *
 * It lives in its own file because two parts need it, and each other too: a tile opens the
 * viewer, and the viewer asks for the same root as the tile.
 */

import { createContext, useContext } from "react";

/** The project root that paths in the feed and the composer are written relative to. */
export const PreviewRoot = createContext<string | null>(null);

/** Open a picture in the viewer. Outside it, does nothing. */
export const OpenImage = createContext<(path: string) => void>(() => {});

export function useOpenImage(): (path: string) => void {
  return useContext(OpenImage);
}
