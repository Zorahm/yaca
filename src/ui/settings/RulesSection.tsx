/**
 * Mode and project rules (§5.7, §8, §10).
 *
 * Two lists, not one with a sign. The questions differ: "what happens without me"
 * and "what never happens" — and a refusal carries a reason, which an allowance does
 * not have. A refusal is stronger than an allowance and stronger than a bypass: the
 * order lives in `Broker::standing`, here it is only named in words.
 *
 * A rule reaches this list by one path only — from here. "Allow always" and "Never
 * allow" in the feed live until the end of the session: what was accepted in a hurry
 * must not outlive the hurry (§5.7).
 */

import { Plus, X } from "@phosphor-icons/react";
import { useState } from "react";

import { refusal, type Refusal, type ScopeKey, type Settings } from "../../core/client";
import { t, type Key } from "../../i18n";
import type { Mode } from "../../core/client";
import { Row, Segments } from "./Row";

/** Scope kinds from §5.7 plus domain and MCP server — both beyond the spec, and
    `permission/mod.rs` marks that. */
const KINDS = ["tool", "directory", "commandPrefix", "domain", "mcpServer"] as const;
type Kind = (typeof KINDS)[number];

const KIND_NAMES: Record<Kind, Key> = {
  tool: "rules.kind_tool",
  directory: "rules.kind_directory",
  commandPrefix: "rules.kind_command",
  domain: "rules.kind_domain",
  mcpServer: "rules.kind_mcp",
};

const KIND_HINTS: Record<Kind, Key> = {
  tool: "rules.hint_tool",
  directory: "rules.hint_directory",
  commandPrefix: "rules.hint_command",
  domain: "rules.hint_domain",
  mcpServer: "rules.hint_mcp",
};

/** The scope's value — one field in all five, only named differently. */
function valueOf(scope: ScopeKey): string {
  switch (scope.type) {
    case "tool":
      return scope.name;
    case "directory":
      return scope.path;
    case "commandPrefix":
      return scope.prefix;
    case "domain":
      return scope.host;
    case "mcpServer":
      return scope.server;
  }
}

function scopeOf(kind: Kind, value: string): ScopeKey {
  switch (kind) {
    case "tool":
      return { type: "tool", name: value };
    case "directory":
      return { type: "directory", path: value };
    case "commandPrefix":
      return { type: "commandPrefix", prefix: value };
    case "domain":
      return { type: "domain", host: value };
    case "mcpServer":
      return { type: "mcpServer", server: value };
  }
}

function same(a: ScopeKey, b: ScopeKey): boolean {
  return a.type === b.type && valueOf(a) === valueOf(b);
}

export function RulesSection({
  settings,
  onPatch,
}: {
  settings: Settings;
  onPatch: (change: (settings: Settings) => Settings) => void;
}) {
  const { rules, denies } = settings.permissions;

  return (
    <>
      <div className="set-head">
        <div>
          <h1 className="set-title">{t("rules.title")}</h1>
          <p className="set-lede">{t("rules.lede")}</p>
        </div>
      </div>

      <div className="set-rows">
        <Row name={t("rules.mode")} hint={t("rules.mode_hint")}>
          <Segments<Mode | "">
            label={t("rules.mode")}
            value={settings.mode ?? ""}
            options={[
              { value: "auto", label: t("mode.auto") },
              { value: "manual", label: t("mode.manual") },
              { value: "edits", label: t("mode.edits") },
              { value: "plan", label: t("mode.plan") },
            ]}
            onChange={(mode) =>
              onPatch((now) => ({ ...now, mode: mode === "" ? null : mode }))
            }
          />
        </Row>
      </div>

      <div className="set-subhead">
        <div>
          <span className="set-subname">{t("rules.allowed")}</span>
          <span className="set-subhint">{t("rules.allowed_hint")}</span>
        </div>
        {rules.length > 0 && (
          <button
            type="button"
            className="btn btn-row"
            onClick={() =>
              onPatch((now) => ({
                ...now,
                permissions: { ...now.permissions, rules: [] },
              }))
            }
          >
            {t("rules.reset")}
          </button>
        )}
      </div>

      <div className="set-list">
        {rules.length === 0 && <div className="set-empty">{t("rules.allowed_empty")}</div>}
        {rules.map((scope, index) => (
          <RuleRow
            key={`${scope.type}:${valueOf(scope)}:${index}`}
            scope={scope}
            onRemove={() =>
              onPatch((now) => ({
                ...now,
                permissions: {
                  ...now.permissions,
                  rules: now.permissions.rules.filter((other) => !same(other, scope)),
                },
              }))
            }
          />
        ))}
      </div>

      <AddRule
        submit={t("rules.allow")}
        onAdd={(scope) =>
          onPatch((now) => ({
            ...now,
            permissions: {
              ...now.permissions,
              rules: [...now.permissions.rules.filter((o) => !same(o, scope)), scope],
            },
          }))
        }
      />

      <div className="set-subhead set-subhead-sep">
        <div>
          <span className="set-subname">{t("rules.denied")}</span>
          <span className="set-subhint">{t("rules.denied_hint")}</span>
        </div>
        {denies.length > 0 && (
          <button
            type="button"
            className="btn btn-row"
            onClick={() =>
              onPatch((now) => ({
                ...now,
                permissions: { ...now.permissions, denies: [] },
              }))
            }
          >
            {t("rules.reset")}
          </button>
        )}
      </div>

      <div className="set-list">
        {denies.length === 0 && <div className="set-empty">{t("rules.denied_empty")}</div>}
        {denies.map((deny, index) => (
          <RuleRow
            key={`${deny.scope.type}:${valueOf(deny.scope)}:${index}`}
            scope={deny.scope}
            reason={deny.reason}
            deny
            onRemove={() =>
              onPatch((now) => ({
                ...now,
                permissions: {
                  ...now.permissions,
                  denies: now.permissions.denies.filter(
                    (other: Refusal) => !same(other.scope, deny.scope),
                  ),
                },
              }))
            }
          />
        ))}
      </div>

      <AddRule
        submit={t("rules.deny")}
        withReason
        onAdd={(scope, reason) =>
          onPatch((now) => ({
            ...now,
            permissions: {
              ...now.permissions,
              // A repeat of the same scope replaces the reason rather than adding a
              // second line: the last word on it is what the agent will read.
              denies: [
                ...now.permissions.denies.filter((o: Refusal) => !same(o.scope, scope)),
                refusal(scope, reason),
              ],
            },
          }))
        }
      />
    </>
  );
}

function RuleRow({
  scope,
  reason,
  deny = false,
  onRemove,
}: {
  scope: ScopeKey;
  reason?: string | null;
  deny?: boolean;
  onRemove: () => void;
}) {
  const kind = scope.type as Kind;
  return (
    <div className={deny ? "rule rule-deny" : "rule"}>
      <span className="rule-kind">{t(KIND_NAMES[kind])}</span>
      <span className="rule-value">{valueOf(scope)}</span>
      <span className="rule-reason">
        {deny ? (reason ?? t("rules.no_reason")) : t(KIND_HINTS[kind])}
      </span>
      <button
        type="button"
        className="btn btn-row"
        aria-label={t("rules.remove")}
        title={t("rules.remove")}
        onClick={onRemove}
      >
        <X size={12} />
      </button>
    </div>
  );
}

/**
 * Adding a rule. Each list has its own form, and there is no "allow or deny" switch
 * in it: the list above it says that.
 */
function AddRule({
  submit,
  withReason = false,
  onAdd,
}: {
  submit: string;
  withReason?: boolean;
  onAdd: (scope: ScopeKey, reason: string) => void;
}) {
  const [kind, setKind] = useState<Kind>("commandPrefix");
  const [value, setValue] = useState("");
  const [reason, setReason] = useState("");

  const add = () => {
    const trimmed = value.trim();
    if (trimmed === "") return;
    onAdd(scopeOf(kind, trimmed), reason);
    setValue("");
    setReason("");
  };

  return (
    <div className="rule-add">
      <Segments<Kind>
        label={t("rules.kind")}
        value={kind}
        options={KINDS.map((k) => ({ value: k, label: t(KIND_NAMES[k]) }))}
        onChange={setKind}
      />
      <input
        className="prov-input prov-input-mono"
        value={value}
        spellCheck={false}
        aria-label={t("rules.value")}
        placeholder={t("rules.value_placeholder")}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && add()}
      />
      {withReason && (
        <input
          className="prov-input"
          value={reason}
          aria-label={t("rules.reason")}
          placeholder={t("rules.reason_placeholder")}
          onChange={(e) => setReason(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
      )}
      <button
        type="button"
        className="btn btn-small"
        disabled={value.trim() === ""}
        onClick={add}
      >
        <Plus size={13} /> {submit}
      </button>
    </div>
  );
}
