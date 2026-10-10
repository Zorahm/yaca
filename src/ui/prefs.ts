/**
 * Настройки приложения, приложенные к окну (§10).
 *
 * Весь внешний вид — три атрибута на `<html>`, значения которых разбирает
 * `tokens.css`. Ни один компонент о теме не знает: она меняет
 * токены, а компоненты читают токены. Отсюда и отсутствие React-контекста —
 * перерисовывать нечего.
 *
 * Умолчание пишется отсутствием атрибута, а не его значением: `:root` — это и
 * есть Бумага (§2), и `data-theme="light"` был бы вторым написанием того же
 * самого. Атрибутом отмечается Ночь.
 *
 * Окон два (§10), и настройки в них общие. Каждое подписывается на канал
 * `yaca://preferences` и применяет то, что пришло: тема, выбранная в
 * настройках, обязана доехать до ленты, иначе окно врёт про состояние (§0).
 */

import { useEffect, useState } from "react";

import { DEFAULT_PREFS, onPreferences, preferences, type Preferences } from "../core/client";
import { setLocale } from "../i18n";
import type { Locale } from "../i18n";

/** Медиазапрос, которым спрашивают систему про тему. */
const DARK = "(prefers-color-scheme: dark)";

function prefersLight(): boolean {
  return typeof matchMedia === "function" ? !matchMedia(DARK).matches : false;
}

/** Светлая ли тема при этих настройках. */
export function isLight(prefs: Preferences): boolean {
  return prefs.theme === "paper" || (prefs.theme === "system" && prefersLight());
}

/** Язык окна при этих настройках: `system` спрашивает вебвью (§14). */
export function localeOf(prefs: Preferences): Locale | null {
  return prefs.language === "system" ? null : prefs.language;
}

/**
 * Ставит атрибуты на `<html>`. Идемпотентно: зовётся и на старте, и на каждое
 * изменение, своё или из соседнего окна.
 */
export function applyPrefs(prefs: Preferences): void {
  const root = document.documentElement;

  if (isLight(prefs)) root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", "dark");

  // Номера строк включены по умолчанию, поэтому атрибутом отмечается их
  // отсутствие — так же, как в `tokens.css`.
  if (prefs.lineNumbers) root.removeAttribute("data-line-numbers");
  else root.setAttribute("data-line-numbers", "off");

  if (prefs.compact) root.setAttribute("data-compact", "on");
  else root.removeAttribute("data-compact");
}

/**
 * Читает настройки и прикладывает их до первой отрисовки.
 *
 * Зовётся из точки входа окна и ожидается: язык выбирается здесь, а словарь
 * читают при первом же рендере. Окно, нарисованное на одном языке и
 * перерисованное на другом, — это мигание на ровном месте.
 *
 * Отказ оболочки не останавливает окно: умолчания — это то, чем приложение
 * было до того, как настройка появилась, и показать ленту важнее, чем тему.
 */
export async function bootPrefs(): Promise<Preferences> {
  let prefs = DEFAULT_PREFS;
  try {
    prefs = await preferences();
  } catch (e) {
    console.error("не прочитать настройки приложения", e);
  }
  setLocale(localeOf(prefs));
  applyPrefs(prefs);
  return prefs;
}

/**
 * Настройки окна: приложенные к `<html>` и переживающие правку из соседнего
 * окна.
 *
 * Язык меняется здесь же, вместе с остальным. Словарь — модульная переменная,
 * и перерисовка всего дерева читает уже новый: перезагружать окно ради языка
 * нельзя, в главном от этого закрылись бы открытые чаты (README, «Чего нет»).
 * Работает это ровно потому, что ни один компонент не мемоизирован — память
 * о старом языке жила бы в пропущенном рендере.
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

  // «Как в системе» слушает систему, а не только соседнее окно.
  useEffect(() => {
    if (prefs.theme !== "system" || typeof matchMedia !== "function") return;
    const query = matchMedia(DARK);
    const react = () => applyPrefs(prefs);
    query.addEventListener("change", react);
    return () => query.removeEventListener("change", react);
  }, [prefs]);

  return prefs;
}
