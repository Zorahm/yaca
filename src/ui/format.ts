/**
 * Numbers and time the way a human reads them.
 *
 * The core delivers milliseconds, bytes and tokens as numbers — “1.2 КБ” is
 * text, and text belongs to the feed (§5.7). It is made here, in the window's
 * language.
 */

import { locale, t } from "../i18n";

/** `14:32` in a checkpoint divider (§5.8). */
export function timeOf(ms: number): string {
  return new Date(ms).toLocaleTimeString(locale, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** The full date and time, for the tooltip over a turn's time line. */
export function dateTimeOf(ms: number): string {
  return new Date(ms).toLocaleString(locale, { dateStyle: "long", timeStyle: "medium" });
}

/** `1 ч 4 мин` — a process's uptime (§6.5). Counted by the UI: the core,
    ticking an event every second, would not say anything new. */
export function durationOf(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours > 0) return `${t("dur.h", { n: hours })} ${t("dur.m", { n: minutes })}`;
  if (minutes > 0) return `${t("dur.m", { n: minutes })} ${t("dur.s", { n: seconds })}`;
  return t("dur.s", { n: seconds });
}

/**
 * `259`, `24.8k`, `1M` — tokens in the context window (§7.1).
 *
 * One decimal and no more: the list under the ring puts 335.9k next to 19k,
 * and rounding the first to 336k would make its rows stop adding up by eye.
 * A whole number drops the `.0`, so a model's window reads `200k`, not
 * `200.0k`.
 */
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  const short = (value: number) => value.toFixed(1).replace(/\.0$/, "");
  // 999 950 and up would round to `1000k`.
  return n < 999_950 ? `${short(n / 1000)}k` : `${short(n / 1_000_000)}M`;
}

/** `39%`, `33.6%` — a share of the context window, from a 0–1 ratio. */
export function percent(ratio: number, digits: number): string {
  return `${(ratio * 100).toFixed(digits)}%`;
}

/** `$0.31`. Four digits for tiny amounts: three cents are not zero. */
export function money(usd: number): string {
  return usd < 0.01 && usd > 0 ? usd.toFixed(4) : usd.toFixed(2);
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/**
 * A file name extracted from a path. The signature shows the short name; the
 * full path lives in the expansion (§5.5).
 *
 * There are two separators: paths inside the change set arrive with a forward
 * slash, while the project root comes from the OS and on Windows uses a
 * backslash. With a single slash the project name `D:\Kod` would never be cut
 * off.
 */
export function baseName(path: string): string {
  const at = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return at === -1 ? path : path.slice(at + 1);
}

/**
 * Путь без расширенного префикса Windows.
 *
 * Ядро хранит корень приведённым, и это осознанно: иначе `D:\Kod` и
 * `\\?\D:\Kod` были бы двумя разными проектами. Но человеку `\\?\` не
 * сообщает ничего, а в запросе разрешения §5.7 он стоит ровно там, где просят
 * вглядеться, — в строке рабочего каталога, где замечают `&& rm -rf`.
 *
 * Снимается только на показ. Сравнивать и сохранять по-прежнему приведённую
 * форму: две правды о том, где лежит файл, разъедутся на первом же сравнении.
 */
export function plainPath(path: string): string {
  // `\\?\UNC\server\share` — это `\\server\share`, а не `server\share`.
  return path.replace(/^\\\\\?\\(UNC\\)?/, (_, unc: string | undefined) =>
    unc ? "\\\\" : "",
  );
}

/**
 * Путь относительно корня проекта, если он внутри, и он же — если нет.
 *
 * Диалог файлов (§7.2) отдаёт абсолютные пути, а чип над полем и карточка
 * §5.1 показывают то же, что видит модель, — путь от корня. Сравнение идёт по
 * приведённой форме: разделители у диалога и у корня разные, а Windows ещё и
 * не различает регистр. Не совпало — путь остаётся абсолютным: соврать про
 * то, где лежит файл, хуже, чем показать его длинно.
 */
export function relativeTo(root: string, path: string): string {
  const flat = (s: string) => s.replace(/\\/g, "/").replace(/\/+$/, "");
  const base = flat(root);
  const full = flat(path);
  const inside = full.toLowerCase().startsWith(`${base.toLowerCase()}/`);
  return inside ? full.slice(base.length + 1) : full;
}
