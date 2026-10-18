/**
 * The Korean dictionary.
 *
 * Each section is checked against the Russian section of the same name, so a
 * key added there and forgotten here is a compile error in the file that
 * misses it rather than a word that shows up in the wrong language on
 * someone's screen.
 *
 * Korean has no plural forms, so `one`, `few` and `many` simply carry the same
 * text. The lookup falls through to them anyway, so every group keeps all three
 * keys.
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

export const ko = merge(sections);
