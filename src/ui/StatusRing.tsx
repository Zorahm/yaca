/**
 * Кольцо состояния (§7.1) — единственный индикатор работы в интерфейсе.
 *
 * Заполнение кольца показывает занятость контекста, наведение даёт токены и
 * деньги. Четыре состояния различаются не только цветом: простой — тонкое
 * кольцо, работа — вращающаяся дуга, ожидание решения — пульс. Форма несёт
 * состояние сама, потому что §14 не разрешает опираться на один цвет.
 */

import type { UsageSnapshot } from "../core/client";
import { t } from "../i18n";
import { money, tokens } from "./format";

export type RingState = "idle" | "busy" | "waiting" | "error";

const R = 6.5;
const CIRCUMFERENCE = 2 * Math.PI * R;

export function StatusRing({
  state,
  usage,
}: {
  state: RingState;
  usage: UsageSnapshot | null;
}) {
  const share =
    usage && usage.contextMax > 0
      ? Math.min(1, usage.contextUsed / usage.contextMax)
      : 0;

  const color =
    state === "error"
      ? "var(--red-ink)"
      : state === "idle"
        ? "var(--line-strong)"
        : state === "waiting"
          ? "var(--red-ink)"
          : "var(--ink)";

  const label =
    state === "busy"
      ? t("ring.busy")
      : state === "waiting"
        ? t("ring.waiting")
        : state === "error"
          ? t("ring.error")
          : t("ring.idle");

  const tooltip = usage
    ? `${label} · ${t("ring.tooltip", {
        used: tokens(usage.contextUsed),
        max: tokens(usage.contextMax),
        cost: money(usage.costUsd),
      })}`
    : label;

  return (
    <svg className="ring" viewBox="0 0 16 16" role="img" aria-label={tooltip}>
      {/* Тултип у SVG — элементом, а не атрибутом: §7.1 просит наведение,
          и `<title>` внутри заодно читается скринридером (§14). */}
      <title>{tooltip}</title>
      <circle cx="8" cy="8" r={R} fill="none" stroke={color} strokeWidth="1.5" opacity="0.35" />

      {/* Заполнение — занятость контекста. */}
      {share > 0 && (
        <circle
          cx="8"
          cy="8"
          r={R}
          fill="none"
          stroke={color}
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeDasharray={`${share * CIRCUMFERENCE} ${CIRCUMFERENCE}`}
          transform="rotate(-90 8 8)"
        />
      )}

      {/* Работа — вращающаяся дуга поверх кольца. */}
      {state === "busy" && (
        <circle
          className="spin"
          cx="8"
          cy="8"
          r={R}
          fill="none"
          stroke="var(--ink)"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeDasharray={`${CIRCUMFERENCE / 4} ${CIRCUMFERENCE}`}
        />
      )}

      {state === "waiting" && (
        <circle
          className="pulse-slow"
          cx="8"
          cy="8"
          r={R}
          fill="none"
          stroke="var(--red-ink)"
          strokeWidth="1.5"
        />
      )}
    </svg>
  );
}
