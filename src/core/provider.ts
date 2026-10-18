/**
 * The `{key}` rule, shared by everything that looks at a provider spec.
 *
 * This mirrors `crates/yaca-core/src/provider/registry.rs`: the placeholder,
 * the protocol's default headers and the "does this provider need a key"
 * rule. The frontend cannot ask the core about a draft (an unsaved provider
 * is not in the registry) and the stand's registry has no core behind it at
 * all, so the same rule is computed here — one implementation for the form
 * and for the stand, not two that drift apart.
 */

import type { Protocol } from "./bindings/Protocol";

/** `registry::ANTHROPIC_ID`: the one preset that speaks the Messages API. */
export const BUILTIN_PROVIDER_ID = "anthropic";

/** `registry::OPENAI_ID`. */
export const OPENAI_PROVIDER_ID = "openai";

/**
 * `registry::DECISION_ID`: where the router's small model lives. An address to
 * be filled in with a local or a cloud server, not a service of its own.
 */
export const DECISION_PROVIDER_ID = "decision";

/**
 * `registry::presets`: the providers Yaca ships with, in the core's order.
 *
 * The window never builds its list from this — the list comes from the core.
 * The table is here for what has no core behind it: the stand's registry, the
 * "is this a preset" rule and the marks in `ui/brand`.
 */
export const PRESETS: ReadonlyArray<{
  id: string;
  displayName: string;
  protocol: Protocol;
  baseUrl: string;
  /** `registry::presets::Preset::keyless`: written down with `authorization` emptied. */
  keyless?: boolean;
}> = [
  {
    id: OPENAI_PROVIDER_ID,
    displayName: "OpenAI",
    protocol: "openai",
    baseUrl: "https://api.openai.com",
  },
  {
    id: BUILTIN_PROVIDER_ID,
    displayName: "Anthropic",
    protocol: "anthropic",
    baseUrl: "https://api.anthropic.com",
  },
  {
    id: "google",
    displayName: "Google AI Studio",
    protocol: "openai",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
  },
  {
    id: "openrouter",
    displayName: "OpenRouter",
    protocol: "openai",
    baseUrl: "https://openrouter.ai/api",
  },
  {
    id: "deepseek",
    displayName: "DeepSeek",
    protocol: "openai",
    baseUrl: "https://api.deepseek.com",
  },
  {
    id: "zai",
    displayName: "z.ai (coding plan)",
    protocol: "openai",
    baseUrl: "https://api.z.ai/api/coding/paas/v4",
  },
  {
    id: DECISION_PROVIDER_ID,
    displayName: "Decision",
    protocol: "openai",
    baseUrl: "http://localhost:11434",
    keyless: true,
  },
];

/** `registry::is_preset`. The core refuses to delete one: the next load would
    put it back, and a delete that undoes itself lies (§0). */
export function isPreset(id: string): boolean {
  return PRESETS.some((preset) => preset.id === id);
}

/** `registry::KEY_PLACEHOLDER`. Substituted from the keychain at request time. */
export const KEY_PLACEHOLDER = "{key}";

/** What each shape sends when the spec says nothing (`registry::protocol_headers`). */
function protocolHeaders(protocol: Protocol): Array<[string, string]> {
  switch (protocol) {
    case "anthropic":
      return [
        ["accept", "text/event-stream"],
        ["anthropic-version", "2023-06-01"],
        ["content-type", "application/json"],
        ["x-api-key", KEY_PLACEHOLDER],
      ];
    case "openai":
      return [
        ["accept", "text/event-stream"],
        ["authorization", `Bearer ${KEY_PLACEHOLDER}`],
        ["content-type", "application/json"],
      ];
  }
}

/**
 * Protocol defaults first, the spec's own over them — the same merge as
 * `ProviderSpec::all_headers`. Overriding a default with an empty value is
 * legitimate: that is how a local gateway drops `authorization` entirely.
 */
export function mergedHeaders(
  protocol: Protocol,
  headers: Array<readonly [string, string]>,
): Array<[string, string]> {
  const merged = new Map<string, string>(protocolHeaders(protocol));
  for (const [name, value] of headers) {
    if (name) merged.set(name, value);
  }
  return [...merged];
}

/**
 * `ProviderSpec::needs_key`: does any outgoing header ask for the key?
 * A provider that never mentions `{key}` needs no key, and the interface must
 * not demand one (§10).
 */
export function needsKey(
  protocol: Protocol,
  headers: Array<readonly [string, string]>,
): boolean {
  return mergedHeaders(protocol, headers).some(([, value]) =>
    value.includes(KEY_PLACEHOLDER),
  );
}
