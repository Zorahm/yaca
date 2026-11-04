import type { ProviderSpec } from "../../core/client";
import type { ProviderPoll } from "../../core/bindings/ProviderPoll";
import { locale, t } from "../../i18n";
import type { KeyState } from "./provider";

export type ProviderStatus = "ready" | "missing_key" | "unasked" | "error" | "asking" | "loading" | "unknown" | "empty";

export function providerStatus(spec: ProviderSpec, key: KeyState, poll: ProviderPoll | undefined, asking = false, error?: string): ProviderStatus {
  if (asking) return "asking";
  if (key.kind === "loading") return "loading";
  if (key.kind === "none") return "missing_key";
  if (key.kind === "unknown") return "unknown";
  if (error || poll?.error) return "error";
  if (poll && spec.models.length === 0) return "empty";
  return spec.models.length > 0 ? "ready" : "unasked";
}

export const STATUS_ORDER: Record<ProviderStatus, number> = {
  error: 0, empty: 1, ready: 2, asking: 3, unasked: 4, missing_key: 5, unknown: 6, loading: 7,
};

export function providerAddress(baseUrl: string): { host: string; local: boolean } {
  try {
    const url = new URL(baseUrl);
    const hostname = url.hostname.toLowerCase().replace(/\.$/, "");
    return {
      host: url.host,
      local: hostname === "localhost" || hostname.endsWith(".localhost") || hostname === "[::1]" || /^127\./.test(hostname),
    };
  } catch {
    return { host: baseUrl.replace(/^https?:\/\//i, ""), local: false };
  }
}

export function pollErrorLabel(error: string): string {
  if (/\b401\b/.test(error)) return t("prov.status_unauthorized");
  if (/\b403\b/.test(error)) return t("prov.status_forbidden");
  if (/network:|timed?\s*out|timeout|connection|connect error/i.test(error)) return t("prov.status_offline");
  return t("prov.status_error");
}

export function pollAge(checkedAt: number, now: number): string {
  const minutes = Math.max(0, Math.floor((now - checkedAt) / 60_000));
  const format = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  if (minutes === 0) return t("prov.checked_now");
  if (minutes < 60) return format.format(-minutes, "minute");
  if (minutes < 1440) return format.format(-Math.floor(minutes / 60), "hour");
  return format.format(-Math.floor(minutes / 1440), "day");
}
