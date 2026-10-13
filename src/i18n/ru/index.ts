/**
 * The Russian dictionary.
 *
 * The source of keys: every other language has to cover it, section by
 * section, or it does not compile.
 *
 * Numerals come in `…one/few/many` groups and are read through `tp()`.
 * Technical terms are not translated by force: `diff`, `commit` and `LSP`
 * stay themselves.
 */

import { merge } from "../merge";
import { composer } from "./composer";
import { feed } from "./feed";
import { mcp } from "./mcp";
import { panels } from "./panels";
import { providers } from "./providers";
import { settings } from "./settings";
import { shell } from "./shell";

export const sections = [shell, feed, composer, panels, settings, providers, mcp] as const;

export const ru = merge(sections);
