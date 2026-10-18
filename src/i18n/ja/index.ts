/**
 * The Japanese dictionary.
 *
 * Each section is checked against the Russian section of the same name, so a
 * key added there and forgotten here is a compile error in the file that
 * misses it rather than a word that shows up in the wrong language on
 * someone's screen.
 *
 * Japanese has no plural forms: the `one`, `few` and `many` keys of a group
 * carry the same text, and the counter word (件, 枚) does the work.
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

export const ja = merge(sections);
