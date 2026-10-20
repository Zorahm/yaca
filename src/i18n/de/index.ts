/**
 * The German dictionary.
 *
 * The source of keys: every other language has to cover it, section by
 * section, or it does not compile.
 *
 * Numerals come in `…one/few/many` groups and are read through `tp()`. German
 * has two plural forms where Russian has three, so `few` and `many` simply
 * carry the same text, as they do in English.
 * Technical terms are not translated by force: `diff`, `commit` and `LSP`
 * stay themselves.
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

export const de = merge(sections);
