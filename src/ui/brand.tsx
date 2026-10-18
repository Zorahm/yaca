/**
 * Other people's marks: the logos of the providers and of the MCP protocol.
 *
 * The interface's own icons are Phosphor, with one stroke across the whole
 * window. A logo was drawn by someone else, and redrawing it with that same
 * stroke would show a mark its owner does not have. So the marks come from
 * Devicons, from its single-colour set: it is painted in `currentColor` and sits
 * on the paper in the same ink as everything else (§2), with no brand colours in
 * the middle of a list.
 *
 * All of them pass through this file, so that a second icon set does not spread
 * across the window: where a mark fits is decided here, not in each section.
 */

// The `…Icon` components are the marks; the bare names are wordmarks, which at
// 15 px are a hairline. DeepSeek and z.ai ship one drawing each, under the
// bare name, and it is the mark.
import {
  AnthropicIcon,
  Deepseek,
  GoogleAiStudioIcon,
  McpIcon,
  OpenaiIcon,
  OpenrouterIcon,
  Zai,
} from "@dev.icons/react/mono";

import type { Protocol } from "../core/client";
import { BUILTIN_PROVIDER_ID, OPENAI_PROVIDER_ID } from "../core/provider";

/** By preset id (`core/provider` `PRESETS`). A map and not an object: a
    provider of one's own may be called `constructor`. */
const PROVIDER_MARKS = new Map<string, typeof OpenaiIcon>([
  [OPENAI_PROVIDER_ID, OpenaiIcon],
  [BUILTIN_PROVIDER_ID, AnthropicIcon],
  ["google", GoogleAiStudioIcon],
  ["openrouter", OpenrouterIcon],
  ["deepseek", Deepseek],
  ["zai", Zai],
]);

/** The MCP protocol mark. Decorative: a label always stands beside it. */
export function McpMark({ size }: { size: number }) {
  return <McpIcon size={size} aria-hidden="true" />;
}

/**
 * The mark of a preset provider, or `null` for one's own.
 *
 * By identifier, not by address or name: presets are recognised exactly the way
 * the core recognises them. One's own gateway with the OpenAI address stays
 * without a mark — it belongs to someone else, and someone else's logo over it
 * would lie.
 */
export function ProviderMark({ id, size }: { id: string; size: number }) {
  const Mark = PROVIDER_MARKS.get(id);
  return Mark === undefined ? null : <Mark size={size} aria-hidden="true" />;
}

/**
 * The protocol as its owner writes it. Proper names are not translated, so they
 * live here and not in the dictionaries: it is the same word in both languages.
 */
export const PROTOCOL_NAME: Record<Protocol, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
};

/** The protocol mark: the request form has the same owner as the mark. */
export function ProtocolMark({ protocol, size }: { protocol: Protocol; size: number }) {
  return protocol === "anthropic" ? (
    <AnthropicIcon size={size} aria-hidden="true" />
  ) : (
    <OpenaiIcon size={size} aria-hidden="true" />
  );
}
