/**
 * A form an MCP server asks the human to fill in (elicitation).
 *
 * While it waits it is the same teal card as a permission request or a
 * question: a decision is owed. The server's name stands on top, because the
 * words below are the server's, not the agent's, and a person should know who
 * is asking before typing anything in. Once answered it collapses into one line
 * of what happened, which stays in the conversation.
 *
 * `⏎` sends when every required field is filled; `Esc` closes the form without
 * deciding, which the server is told as a cancel. "Decline" is the explicit no.
 */

import { useEffect, useState } from "react";

import type { ElicitField, ElicitationReply, ElicitationRequest } from "../core/client";
import type { ElicitationStatus } from "../core/feed";
import { t } from "../i18n";

type Values = Record<string, unknown>;

export function Elicitation({
  request,
  status,
  onAnswer,
}: {
  request: ElicitationRequest;
  status: ElicitationStatus;
  onAnswer: (reply: ElicitationReply) => void;
}) {
  if (status !== "open") return <Settled request={request} status={status} />;
  return <Open request={request} onAnswer={onAnswer} />;
}

function initial(fields: ElicitField[]): Values {
  const values: Values = {};
  for (const field of fields) {
    if (field.default !== null && field.default !== undefined) {
      values[field.name] = field.kind === "choice" ? String(field.default) : field.default;
    } else if (field.kind === "boolean") {
      values[field.name] = false;
    }
  }
  return values;
}

function filled(value: unknown): boolean {
  return value !== undefined && value !== null && !(typeof value === "string" && value.trim() === "");
}

function Open({
  request,
  onAnswer,
}: {
  request: ElicitationRequest;
  onAnswer: (reply: ElicitationReply) => void;
}) {
  const [values, setValues] = useState<Values>(() => initial(request.fields));
  const ready = request.fields.every((f) => !f.required || filled(values[f.name]));
  const set = (name: string, value: unknown) => setValues((now) => ({ ...now, [name]: value }));

  const submit = () => {
    if (!ready) return;
    const sent: Values = {};
    for (const [name, value] of Object.entries(values)) {
      if (filled(value)) sent[name] = value;
    }
    onAnswer({ type: "accept", values: sent });
  };

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.isComposing) return;
      if (event.key === "Escape") {
        event.preventDefault();
        onAnswer({ type: "cancel" });
      } else if (event.key === "Enter" && !event.shiftKey && ready) {
        event.preventDefault();
        submit();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="ask elicit">
      <div className="ask-target">{t("elicit.from", { server: request.server })}</div>
      {request.message !== "" && (
        <div className="ask-head">
          <div className="ask-question">{request.message}</div>
        </div>
      )}

      <div className="elicit-fields">
        {request.fields.map((field) => (
          <Field
            key={field.name}
            field={field}
            value={values[field.name]}
            onChange={(value) => set(field.name, value)}
          />
        ))}
      </div>

      <div className="ask-actions">
        <button className="btn" onClick={() => onAnswer({ type: "decline" })}>
          {t("elicit.decline")}
        </button>
        <div className="ask-group ask-group-allow">
          <button className="btn btn-accent" disabled={!ready} onClick={submit}>
            {t("elicit.send")}
            <span className="keycap">⏎</span>
          </button>
        </div>
      </div>
    </div>
  );
}

function Field({
  field,
  value,
  onChange,
}: {
  field: ElicitField;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  const id = `elicit-${field.name}`;
  const label = (
    <span className="elicit-label">
      {field.title}
      {field.required && <span className="elicit-required"> *</span>}
    </span>
  );
  const about = field.description ? <span className="ask-note">{field.description}</span> : null;

  if (field.kind === "boolean") {
    return (
      <label className="elicit-field elicit-check" htmlFor={id}>
        <input id={id} type="checkbox" checked={value === true} onChange={(e) => onChange(e.target.checked)} />
        <span className="elicit-text">
          {label}
          {about}
        </span>
      </label>
    );
  }

  let input;
  if (field.kind === "choice") {
    input = (
      <select id={id} value={typeof value === "string" ? value : ""} onChange={(e) => onChange(e.target.value)}>
        <option value="" disabled>
          {t("elicit.choose")}
        </option>
        {field.options.map((option, at) => (
          <option key={option} value={option}>
            {field.optionLabels[at] ?? option}
          </option>
        ))}
      </select>
    );
  } else if (field.kind === "number" || field.kind === "integer") {
    input = (
      <input
        id={id}
        type="number"
        step={field.kind === "integer" ? 1 : "any"}
        value={typeof value === "number" ? value : ""}
        onChange={(e) => {
          const raw = e.target.value;
          onChange(raw === "" ? undefined : field.kind === "integer" ? Math.trunc(Number(raw)) : Number(raw));
        }}
      />
    );
  } else {
    input = (
      <input
        id={id}
        type="text"
        value={typeof value === "string" ? value : ""}
        onChange={(e) => onChange(e.target.value)}
      />
    );
  }

  return (
    <label className="elicit-field" htmlFor={id}>
      {label}
      {about}
      {input}
    </label>
  );
}

function Settled({ request, status }: { request: ElicitationRequest; status: ElicitationStatus }) {
  const outcome =
    status === "accepted"
      ? t("elicit.accepted", { server: request.server })
      : status === "declined"
        ? t("elicit.declined")
        : status === "cancelled"
          ? t("elicit.cancelled")
          : t("elicit.expired");
  return (
    <div className="question-settled">
      <div className="question-settled-q">
        {t("elicit.from", { server: request.server })} {request.message}
      </div>
      <div className="question-settled-a">{outcome}</div>
    </div>
  );
}
