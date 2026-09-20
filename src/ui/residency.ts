/**
 * Which models a local server holds in memory right now (§7.1).
 *
 * Asked each time the model menu opens and never stored: residency is live
 * state, and a mark from a minute ago may already be wrong. Picking a model
 * that is not loaded costs a long load or an error, so the menu says so.
 *
 * A provider whose server said nothing about it is not asked again for the
 * rest of the run. The field is not in the OpenAI shape, and a gateway that
 * lacks it today will lack it on the next open too — while its list can be
 * hundreds of models long.
 */

/** Marks by `residencyKey`: `true` in memory, `false` not; absent is unknown. */
export type Residency = Map<string, boolean>;

export function residencyKey(provider: string, id: string): string {
  return `${provider}\n${id}`;
}

export function createResidency(
  ask: (provider: string) => Promise<Record<string, boolean>>,
): (providers: string[]) => Promise<Residency> {
  const quiet = new Set<string>();

  return async (providers) => {
    const marks: Residency = new Map();
    const asking = [...new Set(providers)].filter((p) => p !== "" && !quiet.has(p));

    await Promise.all(
      asking.map(async (provider) => {
        let answer: Record<string, boolean>;
        try {
          answer = await ask(provider);
        } catch {
          // No answer claims nothing either way, and is not silence: a local
          // server that is not up yet may be up on the next open.
          return;
        }
        const entries = Object.entries(answer);
        if (entries.length === 0) quiet.add(provider);
        for (const [id, loaded] of entries) marks.set(residencyKey(provider, id), loaded);
      }),
    );
    return marks;
  };
}
