/**
 * What the model menu shows for a query (§7.1): the groups, the search and
 * the recently picked.
 *
 * With every preset answering, the list is past a hundred models, and a flat
 * list that long is scrolled rather than read. Three things make it short
 * again: a group per provider, so the eye lands on a name before it lands on
 * a model; a search over what a person remembers of a model — its name, its
 * id or who serves it; and the few models actually in use, first.
 *
 * Pure functions, and the storage behind the recent ones passed in: the menu
 * is the only caller, but this is the part worth testing without it.
 */

import type { ModelInfo } from "../core/client";

/** One model of one provider: the same id is served by several. */
export function modelKey(model: ModelInfo): string {
  return `${model.providerId}\n${model.id}`;
}

export type ModelGroup = {
  /** Provider id; empty for the recent group. */
  provider: string;
  models: ModelInfo[];
};

/**
 * Does every word of the query appear in the model's name, its id or its
 * provider's name?
 *
 * Every word and not the phrase: "open son" is how one asks for Sonnet on
 * OpenRouter, and the two words sit in different fields.
 */
export function matches(model: ModelInfo, providerName: string, query: string): boolean {
  const text = `${model.displayName}\n${model.id}\n${providerName}\n${model.providerId}`.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .every((word) => text.includes(word));
}

/**
 * The models that match, a group per provider, in the order they arrived.
 *
 * The order is the core's (`Providers::all_models`): providers as the panel
 * lists them, each one's models as it answered. A provider none of whose
 * models match has no group — a heading over nothing is noise.
 */
export function groupsOf(
  models: ReadonlyArray<ModelInfo>,
  nameOf: (provider: string) => string,
  query: string,
): ModelGroup[] {
  const groups = new Map<string, ModelInfo[]>();
  for (const model of models) {
    if (!matches(model, nameOf(model.providerId), query)) continue;
    const group = groups.get(model.providerId);
    if (group) group.push(model);
    else groups.set(model.providerId, [model]);
  }
  return [...groups].map(([provider, list]) => ({ provider, models: list }));
}

// --- recently picked --------------------------------------------------------

/** How many are kept. More than a handful stops being a shortcut. */
const RECENT_MAX = 5;

/** `localStorage` item: an interface convenience, like the panel widths. */
export const RECENT_KEY = "yaca.models.recent";

/** The list with this model moved to the front. */
export function withRecent(recent: ReadonlyArray<string>, key: string): string[] {
  return [key, ...recent.filter((other) => other !== key)].slice(0, RECENT_MAX);
}

/**
 * The recent models that are still served, most recent first.
 *
 * A model whose provider was removed, or which the provider stopped listing,
 * simply is not here: offering it would be offering an error.
 */
export function recentOf(
  models: ReadonlyArray<ModelInfo>,
  recent: ReadonlyArray<string>,
): ModelInfo[] {
  const byKey = new Map(models.map((model) => [modelKey(model), model]));
  return recent.flatMap((key) => byKey.get(key) ?? []);
}

/** What is stored, or nothing: a value someone else wrote there is not a list. */
export function parseRecent(stored: string | null): string[] {
  if (stored === null) return [];
  try {
    const value: unknown = JSON.parse(stored);
    return Array.isArray(value)
      ? value.filter((key): key is string => typeof key === "string")
      : [];
  } catch {
    return [];
  }
}
