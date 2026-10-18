/**
 * English strings: the browser column.
 */

import type { browser as source } from "../ru/browser";

export const browser = {
  "browser.toggle": "Browser",
  "browser.back": "Back",
  "browser.forward": "Forward",
  "browser.reload": "Reload",
  "browser.stop": "Stop loading",
  "browser.address": "Address or localhost:port",
  "browser.external": "Open in the system browser",
  "browser.devtools": "Developer tools",
  "browser.empty_title": "Nothing open yet",
  "browser.empty_body": "The page the model or you open will show up here",
  "browser.size_model": "{size} · set by the model",
  "browser.reset_size": "Back to the column's size",
  "browser.resize": "Resize the browser",
  "browser.bad_address": "That does not look like a page address",
  "browser.open_failed": "The page did not open",
  "browser.busy": "The model is using the browser",
} satisfies Record<keyof typeof source, string>;
