/**
 * The context window opened from the ring (§7.1): how full it is, a bar cut
 * into what fills it, and the same parts as a list with tokens and shares.
 *
 * The total is what the provider measured; the split is the core's estimate
 * by the size of each part's text, and the last line says so.
 */

import type { UsageSnapshot } from "../core/client";
import { t, type Key } from "../i18n";
import { partsOf, shareOf, type PartKind } from "./contextParts";
import { money, percent, tokens } from "./format";

const LABEL: Record<PartKind, Key> = {
  messages: "ctx.messages",
  systemPrompt: "ctx.system_prompt",
  tools: "ctx.tools",
  mcpTools: "ctx.mcp_tools",
  mcpNotes: "ctx.mcp_notes",
  instructions: "ctx.instructions",
  memory: "ctx.memory",
  skills: "ctx.skills",
  used: "ctx.used",
  reserved: "ctx.reserved",
  free: "ctx.free",
};

export function ContextWindow({ usage }: { usage: UsageSnapshot }) {
  const parts = partsOf(usage);
  const max = usage.contextMax;
  const of = (n: number) => (max > 0 ? n / max : 0);

  return (
    <div className="menu menu-context" role="dialog" aria-label={t("ctx.title")}>
      <div className="ctx-head">
        <span>{t("ctx.title")}</span>
        <span className="ctx-total">
          {tokens(usage.contextUsed)} / {tokens(max)} ({percent(shareOf(usage), 0)})
        </span>
      </div>

      {/* Decoration: every segment is a row of the list below. */}
      <div className="ctx-bar" aria-hidden="true">
        {parts.map((part) => (
          <span
            key={part.kind}
            className="ctx-seg"
            data-part={part.kind}
            style={{ flexGrow: part.tokens }}
          />
        ))}
      </div>

      <ul className="ctx-list">
        {parts.map((part) => (
          <li key={part.kind} className="ctx-row">
            <span className="ctx-swatch" data-part={part.kind} />
            <span className="ctx-label">{t(LABEL[part.kind])}</span>
            <span className="ctx-num">{tokens(part.tokens)}</span>
            <span className="ctx-num ctx-share">{percent(of(part.tokens), 1)}</span>
          </li>
        ))}
      </ul>

      <div className="ctx-foot">
        <span>{t("ctx.estimate")}</span>
        <span className="ctx-cost">{t("ctx.cost", { cost: money(usage.costUsd) })}</span>
      </div>
    </div>
  );
}
