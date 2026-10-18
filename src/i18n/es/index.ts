/**
 * The Spanish dictionary.
 *
 * Neutral international Spanish in an informal-neutral UI register: buttons
 * are infinitives, and the user is addressed as "tú" only where unavoidable.
 *
 * Each section is checked against the Russian section of the same name, so a
 * key added there and forgotten here is a compile error in the file that
 * misses it rather than a word that shows up in the wrong language on
 * someone's screen.
 *
 * Spanish has two plural forms where Russian has three; `few` and `many` simply
 * carry the same text. That is cheaper than a second key shape, and the lookup
 * falls through to it anyway.
 */

import { merge } from "../merge";
import { composer } from "./composer";
import { feed } from "./feed";
import { mcp } from "./mcp";
import { panels } from "./panels";
import { providers } from "./providers";
import { routing } from "./routing";
import { settings } from "./settings";
import { shell } from "./shell";

export const sections = [shell, feed, composer, panels, settings, providers, mcp, routing] as const;

export const es = merge(sections);
