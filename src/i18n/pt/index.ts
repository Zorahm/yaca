/**
 * The Brazilian Portuguese dictionary.
 *
 * Each section is checked against the Russian section of the same name, so a
 * key added there and forgotten here is a compile error in the file that
 * misses it rather than a word that shows up in the wrong language on
 * someone's screen.
 *
 * Portuguese has two plural forms where Russian has three; `few` and `many`
 * simply carry the same text. `one` is also used for zero, as in "0 arquivo",
 * which is how Brazilian Portuguese reads it.
 */

import { merge } from "../merge";
import { browser } from "./browser";
import { composer } from "./composer";
import { feed } from "./feed";
import { link } from "./link";
import { mcp } from "./mcp";
import { panels } from "./panels";
import { providers } from "./providers";
import { routing } from "./routing";
import { settings } from "./settings";
import { shell } from "./shell";

export const sections = [shell, feed, composer, panels, settings, providers, mcp, routing, browser, link] as const;

export const pt = merge(sections);
