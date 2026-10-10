/**
 * Запрос разрешения (§5.7). Инлайновый блок, не модалка.
 *
 * Четыре яруса, всегда в этом порядке: вопрос, цель кратко, подробность
 * моноширинным, ряд действий. Подробность никогда не обрезается — это то
 * место, где человек замечает `&& rm -rf`.
 *
 * Позиции кнопок неизменны во всех запросах: отклонения слева, разрешения
 * справа. Рука привыкает к позиции, а не читает подписи заново.
 *
 * Цифры работают, пока запрос активен: нажимать `⌃⏎` не обязательно,
 * достаточно `4`. Композер в это время цифры не перехватывает — он ждёт
 * ответа, а не текста.
 *
 * `1` and `Esc` refuse at once, as the button promises. The note for the
 * agent and the standing refusal live one step further, behind `2`: both are
 * a considered "no", and neither should cost the quick one a second key.
 */

import { useEffect, useState } from "react";

import type { PermissionAnswer, PermissionKind, PermissionRequest, ScopeKey } from "../core/client";
import { t, tp, type Key } from "../i18n";
import { bytes, plainPath } from "./format";

const QUESTION: Record<PermissionKind, Key> = {
  runCommand: "ask.command",
  deleteFile: "ask.delete_file",
  deleteDirectory: "ask.delete_dir",
  moveFile: "ask.move_file",
  editFile: "ask.edit_file",
  fetchUrl: "ask.fetch_url",
  mcpWrite: "ask.mcp_write",
};

/** The row, or one of the two steps behind it. */
type Step = "row" | "allowScope" | "deny";

export function Permission({
  request,
  onAnswer,
}: {
  request: PermissionRequest;
  onAnswer: (answer: PermissionAnswer) => void;
}) {
  const [step, setStep] = useState<Step>("row");
  const [reason, setReason] = useState("");

  const note = () => (reason.trim() === "" ? null : reason.trim());
  const allow = () => onAnswer({ type: "allow" });
  const allowAlways = (scope: ScopeKey) => onAnswer({ type: "allowAlways", scope });
  const deny = (why: string | null) => onAnswer({ type: "deny", reason: why });
  const denyAlways = (scope: ScopeKey) => onAnswer({ type: "denyAlways", scope, reason: note() });

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (step === "deny") {
        // The note field has the focus, so a bare digit is text here and a
        // scope takes `⌃` with its digit.
        const at = event.ctrlKey ? Number(event.key) - 1 : -1;
        if (at >= 0 && at < request.scopes.length) {
          event.preventDefault();
          denyAlways(request.scopes[at]);
        } else if (event.key === "Escape") {
          event.preventDefault();
          setStep("row");
        }
        return;
      }

      if (step === "allowScope") {
        const at = Number(event.key) - 1;
        if (at >= 0 && at < request.scopes.length) {
          event.preventDefault();
          allowAlways(request.scopes[at]);
        } else if (event.key === "Escape") {
          event.preventDefault();
          setStep("row");
        }
        return;
      }

      // One refusal button, and it opens the step: `1` does what the button
      // does. `Esc` is not on the button — it refuses at once, the way it stops
      // everything else in the window (§13).
      if (event.key === "Escape") {
        event.preventDefault();
        deny(null);
      } else if (event.key === "1") {
        event.preventDefault();
        setStep("deny");
      } else if (event.key === "2" || (event.ctrlKey && event.shiftKey && event.key === "Enter")) {
        event.preventDefault();
        setStep("allowScope");
      } else if (event.key === "3" || (event.ctrlKey && event.key === "Enter")) {
        event.preventDefault();
        allow();
      }
    }

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="ask">
      <div className="ask-head">
        <div className="ask-question">
          {t(QUESTION[request.kind], { target: request.target })}
        </div>
        {request.total > 1 && (
          <div className="ask-position">
            {t("ask.position", { position: request.position, total: request.total })}
          </div>
        )}
      </div>

      <div className="ask-target">
        {request.target}
        {request.sizeBytes !== null && ` · ${bytes(request.sizeBytes)}`}
      </div>

      <div className="ask-detail">
        {plainPath(request.detail)}
        {request.note && <div className="ask-note">{plainPath(request.note)}</div>}
        {/* §5.7: a directory's full path and the files inside it. */}
        {request.files && (
          <div className="ask-note">
            {tp(request.files.more ? "ask.files_more" : "ask.files", request.files.files)}
          </div>
        )}
      </div>

      {step === "allowScope" && (
        <Scopes
          title={t("ask.allow_scope_title")}
          scopes={request.scopes}
          keycap={(index) => `${index + 1}`}
          onPick={allowAlways}
        />
      )}

      {step === "deny" && (
        <>
          <div className="ask-deny">
            <input
              autoFocus
              value={reason}
              placeholder={t("ask.deny_placeholder")}
              onChange={(e) => setReason(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.ctrlKey) {
                  e.preventDefault();
                  deny(note());
                }
              }}
            />
            <button className="btn" onClick={() => deny(note())}>
              {t("ask.deny_once")}
              <span className="keycap">⏎</span>
            </button>
          </div>
          <Scopes
            title={t("ask.deny_scope_title")}
            scopes={request.scopes}
            keycap={(index) => `⌃${index + 1}`}
            onPick={denyAlways}
          />
        </>
      )}

      {step === "row" && (
        <div className="ask-actions">
          {/* Refusal on the left, consents on the right, and only the consent
              group wraps as a whole, at any window width. */}
          <button className="btn" onClick={() => setStep("deny")}>
            {t("ask.deny")}
            <span className="keycap">1</span>
          </button>
          <div className="ask-group ask-group-allow">
            <button className="btn" onClick={() => setStep("allowScope")}>
              {t("ask.always")}
              <span className="keycap">2</span>
              <span className="keycap">⌃⇧⏎</span>
            </button>
            <button className="btn btn-accent" onClick={allow}>
              {t("ask.allow")}
              <span className="keycap">3</span>
              <span className="keycap">⌃⏎</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/** The choice of how far a standing answer reaches, narrowest first (§5.7). */
function Scopes({
  title,
  scopes,
  keycap,
  onPick,
}: {
  title: string;
  scopes: ScopeKey[];
  keycap: (index: number) => string;
  onPick: (scope: ScopeKey) => void;
}) {
  return (
    <div className="ask-scopes">
      <div className="ask-scope-title">{title}</div>
      {scopes.map((scope, index) => (
        <button className="menu-item" key={index} onClick={() => onPick(scope)}>
          <span className="keycap">{keycap(index)}</span>
          <span>{scopeLabel(scope)}</span>
        </button>
      ))}
    </div>
  );
}

function scopeLabel(scope: ScopeKey): string {
  switch (scope.type) {
    case "tool":
      return t("ask.scope.tool", { name: scope.name });
    case "commandPrefix":
      return t("ask.scope.command_prefix", { prefix: scope.prefix });
    case "domain":
      return t("ask.scope.domain", { host: scope.host });
    case "directory":
      return scope.path === ""
        ? t("ask.scope.directory_root")
        : t("ask.scope.directory", { path: scope.path });
  }
}
