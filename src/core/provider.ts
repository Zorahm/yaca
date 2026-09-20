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

/** `registry::ANTHROPIC_ID`. The first preset, and what a session starts on
    when neither a provider nor a model is named. */
export const BUILTIN_PROVIDER_ID = "anthropic";

/** `registry::OPENAI_ID`. The second preset — everything that is not the
    Messages API speaks this shape. */
export const OPENAI_PROVIDER_ID = "openai";

/** `registry::is_preset`. The core refuses to delete either: the next load
    would put it back, and a delete that undoes itself lies (§0). */
export function isPreset(id: string): boolean {
  return id === BUILTIN_PROVIDER_ID || id === OPENAI_PROVIDER_ID;
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
