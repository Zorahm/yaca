/**
 * The system menu: the bar under the title where "File" usually is.
 *
 * It holds one menu, Help, with two items. Built from the webview through
 * Tauri's menu API rather than in the shell, for two reasons: the labels come
 * from the same dictionaries as the rest of the window and change with the
 * language, and what the items do — open a screen — is the window's business
 * and needs no channel back from Rust.
 *
 * This is the menu of a window that keeps the system's frame — macOS and
 * Linux. On Windows the window draws its own title bar and Help lives there
 * (`./frame`, `ui/TitleBar`). The stand has no system menu to put anything in.
 */

// Type only: the module itself is loaded on demand, and never in the stand.
import type { Submenu as HelpSubmenu } from "@tauri-apps/api/menu";

import { inTauri } from "./demo";
import { isMobile } from "./link";
import { ownTitleBar } from "./frame";

export type HelpMenu = {
  /** The menu's own name. */
  title: string;
  items: Array<{ id: string; text: string; action: () => void }>;
};

/**
 * Puts the Help menu in place, replacing the one from a previous call.
 *
 * On macOS the menu bar is the system's and carries Edit, without which
 * `⌘C` and `⌘V` stop working in the webview — so the default menu is kept and
 * the items go into its own Help. On Linux there is no default worth keeping,
 * and Help is the whole bar.
 */
export async function setHelpMenu(help: HelpMenu): Promise<void> {
  // A window that draws its own title bar has Help in it (`ui/TitleBar`), and
  // a system menu bar under that would be the second strip the bar replaced.
  // A phone has no menu bar to put Help into.
  if (!inTauri || ownTitleBar || isMobile) return;
  const { Menu, MenuItem, Submenu } = await import("@tauri-apps/api/menu");

  const items = await Promise.all(
    help.items.map((item) =>
      MenuItem.new({ id: item.id, text: item.text, action: item.action }),
    ),
  );

  if (navigator.userAgent.includes("Mac")) {
    const menu = await Menu.default();
    // The default Help is found by the name Tauri gives it; if a later
    // version renames it, the items get a menu of their own instead of
    // disappearing.
    let own: HelpSubmenu | null = null;
    for (const entry of await menu.items()) {
      if (entry instanceof Submenu && (await entry.text()) === "Help") own = entry;
    }
    if (own !== null) await own.append(items);
    else await menu.append(await Submenu.new({ text: help.title, items }));
    await menu.setAsAppMenu();
    return;
  }

  const menu = await Menu.new({
    items: [await Submenu.new({ text: help.title, items })],
  });
  await menu.setAsAppMenu();
}

/** The version of the running application, as its manifest states it. */
export async function appVersion(): Promise<string> {
  if (!inTauri) {
    // The stand has no binary; the package it is served from is the nearest
    // thing with a version.
    return (await import("../../package.json")).version;
  }
  return (await import("@tauri-apps/api/app")).getVersion();
}
