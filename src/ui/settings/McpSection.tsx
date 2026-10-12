/**
 * MCP servers in the settings window (§10): one list, shown twice.
 *
 * The same section stands in both halves of the navigation — the human's own
 * servers, for every project, and one project's. They differ in where the list
 * is kept, which the caller knows, and in nothing the section draws.
 *
 * A server is a document, like a provider: it has Save and Cancel, because a
 * half-typed command must not reach a file the next chat starts programs from.
 * The switch on a row is the one thing applied at once.
 *
 * What is declared here is not what is running. A chat reads these lists when
 * it opens and keeps the servers it started with; their state — connecting,
 * ready, failed and why — is in the chat, under `/mcp`.
 */

import { CaretLeft, PencilSimple, Plus, X } from "@phosphor-icons/react";
import { useState } from "react";

import type { McpServerSettings } from "../../core/client";
import { t } from "../../i18n";
import { Segments, Switch } from "./Row";
import {
  draftOf,
  emptyDraft,
  problemsOf,
  settingsOf,
  targetOf,
  type McpDraft,
  type McpKind,
} from "./mcp";

export type McpServers = { [key in string]?: McpServerSettings };

type Editing = { draft: McpDraft; isNew: boolean };

export function McpSection({
  title,
  lede,
  servers,
  error,
  inherited,
  onSave,
}: {
  title: string;
  lede: string;
  /** `null` — not read yet, or could not be read; see `error`. */
  servers: McpServers | null;
  error: string | null;
  /** The project's `.mcp.json`: shown, never written here. */
  inherited?: McpServers;
  onSave: (next: McpServers) => void;
}) {
  const [editing, setEditing] = useState<Editing | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  if (editing !== null && servers !== null) {
    return (
      <>
        <button type="button" className="prov-back" onClick={() => setEditing(null)}>
          <CaretLeft size={13} /> {title}
        </button>
        <McpForm
          key={editing.isNew ? "new" : editing.draft.name}
          initial={editing.draft}
          isNew={editing.isNew}
          taken={Object.keys(servers)}
          onCancel={() => setEditing(null)}
          onSave={(draft) => {
            onSave({ ...servers, [draft.name.trim()]: settingsOf(draft) });
            setEditing(null);
          }}
        />
      </>
    );
  }

  const names = servers === null ? [] : Object.keys(servers).sort();
  const without = (name: string): McpServers => {
    const next = { ...servers };
    delete next[name];
    return next;
  };

  return (
    <>
      <div className="set-head">
        <div>
          <h1 className="set-title">{title}</h1>
          <p className="set-lede">{lede}</p>
        </div>
        {servers !== null && (
          <button
            type="button"
            className="btn btn-small"
            onClick={() => setEditing({ draft: emptyDraft(), isNew: true })}
          >
            <Plus size={14} /> {t("mcp.add")}
          </button>
        )}
      </div>

      {error !== null && <div className="set-error">{error}</div>}

      {servers !== null && names.length === 0 && (
        <div className="set-empty">{t("mcp.none_declared")}</div>
      )}

      {servers !== null && names.length > 0 && (
        <div className="prov-list">
          {names.map((name) => {
            const settings = servers[name];
            if (settings === undefined) return null;
            return (
              <div className="prov" key={name}>
                <button
                  type="button"
                  className="prov-main"
                  title={t("mcp.edit")}
                  onClick={() => setEditing({ draft: draftOf(name, settings), isNew: false })}
                >
                  <span className="prov-name">{name}</span>
                  <span className="prov-meta">{targetOf(settings)}</span>
                </button>

                <span className="prov-proto">{settings.url !== null ? "http" : "stdio"}</span>

                <div className="prov-actions">
                  <Switch
                    on={!settings.disabled}
                    label={t("mcp.enabled", { name })}
                    onChange={(on) =>
                      onSave({ ...servers, [name]: { ...settings, disabled: !on } })
                    }
                  />
                  <button
                    type="button"
                    className="btn btn-row"
                    aria-label={t("mcp.edit")}
                    title={t("mcp.edit")}
                    onClick={() => setEditing({ draft: draftOf(name, settings), isNew: false })}
                  >
                    <PencilSimple size={14} />
                  </button>
                  {confirming === name ? (
                    <button
                      type="button"
                      className="btn btn-row btn-danger"
                      onClick={() => {
                        setConfirming(null);
                        onSave(without(name));
                      }}
                    >
                      {t("mcp.delete_confirm")}
                    </button>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-row btn-danger"
                      aria-label={t("mcp.delete")}
                      title={t("mcp.delete")}
                      onClick={() => setConfirming(name)}
                      onBlur={() => setConfirming(null)}
                    >
                      <X size={14} />
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {inherited !== undefined && Object.keys(inherited).length > 0 && (
        <section className="prov-section">
          <span className="prov-label">{t("mcp.file_title")}</span>
          <div className="prov-hint">{t("mcp.file_hint")}</div>
          <div className="prov-list">
            {Object.keys(inherited)
              .sort()
              .map((name) => {
                const settings = inherited[name];
                if (settings === undefined) return null;
                // A server of the same name declared above replaces this one
                // whole, and the row says so rather than listing a server that
                // will never start.
                const replaced = servers !== null && name in servers;
                return (
                  <div className="prov" key={name}>
                    <div className="prov-main">
                      <span className="prov-name">{name}</span>
                      <span className="prov-meta">
                        {replaced ? t("mcp.file_replaced") : targetOf(settings)}
                      </span>
                    </div>
                    <span className="prov-proto">{settings.url !== null ? "http" : "stdio"}</span>
                  </div>
                );
              })}
          </div>
        </section>
      )}

      <div className="prov-note">
        <span className="prov-dot" aria-hidden="true" />
        <span>{t("mcp.next_chat_note")}</span>
      </div>
    </>
  );
}

const KINDS: McpKind[] = ["stdio", "http"];

function McpForm({
  initial,
  isNew,
  taken,
  onSave,
  onCancel,
}: {
  initial: McpDraft;
  isNew: boolean;
  taken: string[];
  onSave: (draft: McpDraft) => void;
  onCancel: () => void;
}) {
  const [draft, setDraft] = useState(initial);
  const patch = (change: Partial<McpDraft>) => setDraft((now) => ({ ...now, ...change }));
  const problems = problemsOf(draft, taken, isNew);

  return (
    <div className="prov-form">
      <div className="prov-form-head">{isNew ? t("mcp.form_new") : draft.name}</div>

      <div className="prov-grid">
        <label className="prov-field">
          <span className="prov-label">{t("mcp.form_name")}</span>
          {/* The name is the identity: it is part of every tool's name and of
              every rule that names one. Renaming would orphan those. */}
          <input
            className="prov-input prov-input-mono"
            value={draft.name}
            readOnly={!isNew}
            aria-readonly={!isNew}
            spellCheck={false}
            onChange={(e) => patch({ name: e.target.value })}
          />
          <span className="prov-hint">{t("mcp.form_name_hint")}</span>
        </label>
        <div className="prov-field" role="group" aria-label={t("mcp.form_kind")}>
          <span className="prov-label">{t("mcp.form_kind")}</span>
          <Segments<McpKind>
            label={t("mcp.form_kind")}
            value={draft.kind}
            options={KINDS.map((kind) => ({
              value: kind,
              label: t(kind === "stdio" ? "mcp.kind_stdio" : "mcp.kind_http"),
            }))}
            onChange={(kind) => patch({ kind })}
          />
        </div>
      </div>

      {draft.kind === "stdio" ? (
        <>
          <label className="prov-field">
            <span className="prov-label">{t("mcp.form_command")}</span>
            <input
              className="prov-input prov-input-mono"
              value={draft.command}
              spellCheck={false}
              placeholder="npx"
              onChange={(e) => patch({ command: e.target.value })}
            />
            <span className="prov-hint">{t("mcp.form_command_hint")}</span>
          </label>
          <Area
            label={t("mcp.form_args")}
            hint={t("mcp.form_args_hint")}
            value={draft.args}
            onChange={(args) => patch({ args })}
          />
          <Area
            label={t("mcp.form_env")}
            hint={t("mcp.form_env_hint")}
            value={draft.env}
            placeholder="API_TOKEN=${API_TOKEN}"
            onChange={(env) => patch({ env })}
          />
          <label className="prov-field">
            <span className="prov-label">{t("mcp.form_cwd")}</span>
            <input
              className="prov-input prov-input-mono"
              value={draft.cwd}
              spellCheck={false}
              onChange={(e) => patch({ cwd: e.target.value })}
            />
            <span className="prov-hint">{t("mcp.form_cwd_hint")}</span>
          </label>
        </>
      ) : (
        <>
          <label className="prov-field">
            <span className="prov-label">{t("mcp.form_url")}</span>
            <input
              className="prov-input prov-input-mono"
              value={draft.url}
              spellCheck={false}
              placeholder="https://example.com/mcp"
              onChange={(e) => patch({ url: e.target.value })}
            />
            <span className="prov-hint">{t("mcp.form_url_hint")}</span>
          </label>
          <Area
            label={t("mcp.form_headers")}
            hint={t("mcp.form_headers_hint")}
            value={draft.headers}
            placeholder="Authorization: Bearer ${API_TOKEN}"
            onChange={(headers) => patch({ headers })}
          />
        </>
      )}

      {problems.length > 0 && (
        <div className="prov-problems">
          {problems.map((problem) => (
            <div key={problem}>{t(problem)}</div>
          ))}
        </div>
      )}

      <div className="prov-form-foot">
        <button type="button" className="btn" onClick={onCancel}>
          {t("prov.form_cancel")}
        </button>
        <button
          type="button"
          className="btn btn-accent"
          disabled={problems.length > 0}
          onClick={() => onSave(draft)}
        >
          {t("mcp.form_save")}
        </button>
      </div>
    </div>
  );
}

function Area({
  label,
  hint,
  value,
  placeholder,
  onChange,
}: {
  label: string;
  hint: string;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="prov-field">
      <span className="prov-label">{label}</span>
      <textarea
        className="prov-input prov-input-mono prov-area"
        value={value}
        rows={Math.min(8, Math.max(2, value.split("\n").length))}
        spellCheck={false}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
      <span className="prov-hint">{hint}</span>
    </label>
  );
}
