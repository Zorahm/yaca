/**
 * Draft types and validation for the providers panel (§10).
 *
 * The `{key}` rule itself lives in `core/provider` — the one mirror of
 * `registry.rs`, shared with the stand's registry. This module is what the
 * form edits: drafts, and the problems that keep a draft from becoming a
 * spec.
 *
 * Models are not drafted at all any more. They come from the provider itself
 * (`provider::models`), which knows its own context windows, output caps and
 * `effort` support better than anyone typing them in, so the draft carries the
 * answer as it arrived and the form only shows it.
 */

import type { Key, Params } from "../../i18n";
import {
  mergedHeaders as mergedHeaderPairs,
  needsKey as needsKeyPairs,
  KEY_PLACEHOLDER,
  isPreset,
} from "../../core/provider";
import type { ModelInfo, Protocol, ProviderSpec } from "../../core/client";

export { KEY_PLACEHOLDER, isPreset };

/** Header rows as the editor holds them: a name-less row is a validation
    problem, not a header, so it never reaches the merge. */
export function mergedHeaders(
  protocol: Protocol,
  headers: Array<HeaderDraft>,
): Array<[string, string]> {
  return mergedHeaderPairs(
    protocol,
    headers
      .filter((header) => header.name.trim() !== "")
      .map((header) => [header.name.trim(), header.value] as const),
  );
}

/** `ProviderSpec::needs_key` over the same draft rows. */
export function needsKey(
  protocol: Protocol,
  headers: Array<HeaderDraft>,
): boolean {
  return needsKeyPairs(
    protocol,
    headers
      .filter((header) => header.name.trim() !== "")
      .map((header) => [header.name.trim(), header.value] as const),
  );
}

// --- drafts -----------------------------------------------------------------

export type HeaderDraft = { name: string; value: string };

export type ProviderDraft = {
  id: string;
  displayName: string;
  protocol: Protocol;
  baseUrl: string;
  headers: Array<HeaderDraft>;
  /** As the provider reported them (`discoverModels`), or as they were saved. */
  models: Array<ModelInfo>;
};

/** A provider that does not exist yet. OpenAI is the common custom gateway. */
export function emptyDraft(): ProviderDraft {
  return {
    id: "",
    displayName: "",
    protocol: "openai",
    baseUrl: "",
    headers: [],
    models: [],
  };
}

export function draftFromSpec(spec: ProviderSpec): ProviderDraft {
  return {
    id: spec.id,
    displayName: spec.displayName,
    protocol: spec.protocol,
    baseUrl: spec.baseUrl,
    headers: Object.entries(spec.headers).map(([name, value]) => ({
      name,
      value: value ?? "",
    })),
    models: spec.models,
  };
}

// --- validation -------------------------------------------------------------

/** A problem is a dictionary key with its holes, not a rendered string: the
    language of the window is the panel's business, not this module's. */
export type Problem = { key: Key; params?: Params };

function validate(draft: ProviderDraft): {
  problems: Array<Problem>;
  spec: ProviderSpec | null;
} {
  const problems: Array<Problem> = [];

  if (!draft.id.trim()) problems.push({ key: "prov.error_id" });
  if (!draft.displayName.trim()) problems.push({ key: "prov.error_name" });
  if (!draft.baseUrl.trim()) problems.push({ key: "prov.error_url" });

  const names = new Set<string>();
  draft.headers.forEach((header, index) => {
    const name = header.name.trim();
    if (!name) {
      problems.push({ key: "prov.error_header_name", params: { n: index + 1 } });
      return;
    }
    if (names.has(name)) {
      problems.push({ key: "prov.error_header_dup", params: { name } });
    }
    names.add(name);
  });

  return {
    problems,
    spec:
      problems.length > 0
        ? null
        : {
            id: draft.id.trim(),
            displayName: draft.displayName.trim(),
            protocol: draft.protocol,
            baseUrl: draft.baseUrl.trim(),
            headers: Object.fromEntries(
              draft.headers
                // A name-less row is already reported as a problem; the spec
                // just never sees it. Values pass through as typed: an empty
                // value overrides a protocol default with nothing, and that
                // is a legitimate way to drop a header.
                .map((header) => [header.name.trim(), header.value])
                .filter(([name]) => name !== ""),
            ),
            // Как приехали от провайдера: перепроверять его же ответ
            // означало бы иметь про модели своё мнение (§0).
            models: draft.models,
          },
  };
}

export function draftProblems(draft: ProviderDraft): Array<Problem> {
  return validate(draft).problems;
}

/** The spec to save, or `null` while the draft has problems. */
export function draftToSpec(draft: ProviderDraft): ProviderSpec | null {
  return validate(draft).spec;
}

/** Where the provider's key stands, as the shell reports it (`apiKeySource`). */
export type KeyState =
  | { kind: "loading" }
  | { kind: "unknown" }
  | { kind: "not_needed" }
  | { kind: "keychain" }
  | { kind: "environment" }
  | { kind: "none" };
