/**
 * App preferences, applied to the window (§10).
 *
 * All appearance is three attributes on `<html>`, whose values `tokens.css`
 * parses. No component knows about the theme: it changes the tokens, and
 * components read the tokens. Hence no React context either: there is nothing
 * to re-render.
 *
 * The default is written as the absence of an attribute, not as its value:
 * `:root` is already Paper (§2), and `data-theme="light"` would be a second
 * spelling of the same thing. The attribute marks Night.
 *
 * There are two windows (§10) and they share the preferences. Each subscribes
 * to the `yaca://preferences` channel and applies what arrives: a theme chosen
 * in settings must reach the feed, or the window lies about its state (§0).
 */

import { useEffect, useState } from "react";

import { DEFAULT_PREFS, onPreferences, preferences, type Preferences } from "../core/client";
import { setLocale } from "../i18n";
import type { Locale } from "../i18n";

/** The media query that asks the system for the theme. */
const DARK = "(prefers-color-scheme: dark)";

function prefersLight(): boolean {
  return typeof matchMedia === "function" ? !matchMedia(DARK).matches : false;
}

/** Whether the theme is light under these preferences. */
export function isLight(prefs: Preferences): boolean {
  return prefs.theme === "paper" || (prefs.theme === "system" && prefersLight());
}

/** The window's language under these preferences: `system` asks the webview (§14). */
export function localeOf(prefs: Preferences): Locale | null {
  return prefs.language === "system" ? null : prefs.language;
}

/**
 * Sets the attributes on `<html>`. Idempotent: called on start-up and on every
 * change, our own or one from the other window.
 */
export function applyPrefs(prefs: Preferences): void {
  const root = document.documentElement;

  if (isLight(prefs)) root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", "dark");

  // Line numbers are on by default, so the attribute marks their absence,
  // the same way as in `tokens.css`.
  if (prefs.lineNumbers) root.removeAttribute("data-line-numbers");
  else root.setAttribute("data-line-numbers", "off");

  if (prefs.compact) root.setAttribute("data-compact", "on");
  else root.removeAttribute("data-compact");
}

/**
 * Reads the preferences and applies them before the first render.
 *
 * Called from the window's entry point and awaited: the language is chosen
 * here, and the dictionary is read on the very first render. A window drawn in
 * one language and redrawn in another is a flicker for no reason.
 *
 * A failure from the shell does not stop the window: the defaults are what the
 * app was before the setting existed, and showing the feed matters more than
 * the theme.
 */
export async function bootPrefs(): Promise<Preferences> {
  let prefs = DEFAULT_PREFS;
  try {
    prefs = await preferences();
  } catch (e) {
    console.error("could not read app preferences", e);
  }
  setLocale(localeOf(prefs));
  applyPrefs(prefs);
  return prefs;
}

/**
 * Window preferences: applied to `<html>` and kept in sync with edits made in
 * the other window.
 *
 * The language changes here too, together with the rest. The dictionary is a
 * module-level variable, and re-rendering the whole tree reads the new one: the
 * window cannot be reloaded for a language change, because in the main window
 * that would close open chats (README). This works precisely
 * because no component is memoised: a memo would keep the old language alive in
 * a skipped render.
 */
export function usePreferences(initial: Preferences): Preferences {
  const [prefs, setPrefs] = useState(initial);

  useEffect(() => {
    let live = true;
    const off = onPreferences((next) => {
      if (!live) return;
      setLocale(localeOf(next));
      applyPrefs(next);
      setPrefs(next);
    });
    return () => {
      live = false;
      void off.then((stop) => stop());
    };
  }, []);

  // "System" listens to the OS too, not only to the other window.
  useEffect(() => {
    if (prefs.theme !== "system" || typeof matchMedia !== "function") return;
    const query = matchMedia(DARK);
    const react = () => applyPrefs(prefs);
    query.addEventListener("change", react);
    return () => query.removeEventListener("change", react);
  }, [prefs]);

  return prefs;
}
