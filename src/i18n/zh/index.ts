/**
 * The Simplified Chinese dictionary.
 *
 * Chinese has no plural forms, so each `…one/few/many` group carries the same
 * text under all three keys. The keys are kept anyway, so the lookup can stay
 * the same for every language.
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

export const zh = merge(sections);
