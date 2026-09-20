/**
 * What the edoC build left in `localStorage`, brought over to the new names.
 *
 * The composer drafts (§7.4) were kept as `edoc.draft.<chat>`. The profile
 * they live in is moved by the shell (`src-tauri/src/profile.rs`); the keys
 * belong to the page, and are renamed here once, before anything reads them.
 *
 * The page's half of `crates/yaca-core/src/legacy.rs`, and goes with it.
 */

const OLD_PREFIX = "edoc.";
const NEW_PREFIX = "yaca.";

/**
 * Renames every key under the old prefix. A value already under the new name
 * wins — it was typed after the rename — and the old one is only dropped.
 */
export function adoptStorage(storage: Storage = localStorage): void {
  // Collected first: removing while walking by index would skip keys.
  const old: string[] = [];
  for (let i = 0; i < storage.length; i += 1) {
    const key = storage.key(i);
    if (key?.startsWith(OLD_PREFIX)) old.push(key);
  }

  for (const key of old) {
    const renamed = NEW_PREFIX + key.slice(OLD_PREFIX.length);
    const value = storage.getItem(key);
    if (value !== null && storage.getItem(renamed) === null) {
      storage.setItem(renamed, value);
    }
    storage.removeItem(key);
  }
}
