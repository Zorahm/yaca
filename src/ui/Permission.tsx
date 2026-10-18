/**
 * Permission request (§5.7). An inline block, not a modal.
 *
 * Four tiers, always in this order: the question, the target briefly, the detail
 * in monospace, the row of actions. The detail is never cut — this is the place
 * where the person notices `&& rm -rf`.
 *
 * The button positions are fixed in all requests: refusals on the left, allows on
 * the right. The hand gets used to the position rather than reading the labels
 * again.
 *
 * The digits work while the request is active: pressing `⌃⏎` is not required,
 * `4` is enough. The composer does not intercept the digits at this time — it is
 * waiting for an answer, not for text.
 *
 * `1` and `Esc` refuse at once, as the button promises. The note for the
 * agent and the standing refusal live one step further, behind `2`: both are
 * a considered "no", and neither should cost the quick one a second key.
 */

import { useEffect, useState } from "react";

import type {
  PermissionAnswer,
  PermissionKind,
  PermissionRequest,
  ScopeKey,
  Unsaved,
} from "../core/client";
import { t, tp, type Key } from "../i18n";
import { bytes, plainPath } from "./format";
import { alwaysOffered, allowDigit, INVARIANT_LINE, rowAnswer, unsavedLines } from "./permissionRows";

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

  // An invariant comes with no scopes (§8): there is no rule the human could
  // grant that would apply, so "always" is not offered and the row shrinks to
  // refusal and consent. The consent takes the digit that is left.
  const offered = alwaysOffered(request);

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
        return;
      }
      const answer = rowAnswer(
        { key: event.key, ctrl: event.ctrlKey, shift: event.shiftKey },
        offered,
        allowDigit(request),
      );
      if (answer === "none") return;
      event.preventDefault();
      if (answer === "deny") setStep("deny");
      else if (answer === "always") setStep("allowScope");
      else allow();
    }

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="ask">
      <div className="ask-head">
        <div className="ask-question">
          {/* §5.7 names the server in the one question about MCP: "change data in
              Asana". The tool is the line below, as the target. */}
          {t(QUESTION[request.kind], {
            target: request.target,
            server: request.server ?? request.target,
          })}
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

      {request.invariant && (
        <div className="ask-invariant">{t(INVARIANT_LINE[request.invariant])}</div>
      )}

      <div className="ask-detail">
        {plainPath(request.detail)}
        {request.note && <div className="ask-note">{plainPath(request.note)}</div>}
        {/* §5.7: a directory's full path and the files inside it. */}
        {request.files && (
          <div className="ask-note">
            {tp(request.files.more ? "ask.files_more" : "ask.files", request.files.files)}
          </div>
        )}
        {request.unsaved && <UnsavedList unsaved={request.unsaved} />}
      </div>

      {step === "allowScope" && offered && (
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
          {offered && (
            <Scopes
              title={t("ask.deny_scope_title")}
              scopes={request.scopes}
              keycap={(index) => `⌃${index + 1}`}
              onPick={denyAlways}
            />
          )}
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
            {offered && (
              <button className="btn" onClick={() => setStep("allowScope")}>
                {t("ask.always")}
                <span className="keycap">2</span>
                <span className="keycap">⌃⇧⏎</span>
              </button>
            )}
            <button className="btn btn-accent" onClick={allow}>
              {t("ask.allow")}
              <span className="keycap">{allowDigit(request)}</span>
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

/**
 * What a directory delete takes away for good (§5.7): the directories the
 * snapshot skips, and the files it does not keep. The closing line is the
 * promise of the block: a rollback does not bring these back.
 */
function UnsavedList({ unsaved }: { unsaved: Unsaved }) {
  const lines = unsavedLines(unsaved);
  return (
    <div className="ask-unsaved">
      <div className="ask-unsaved-title">{t("ask.unsaved_title")}</div>
      <ul className="ask-unsaved-list">
        {lines.dirs.map((dir) => (
          <li key={dir}>{plainPath(dir)}</li>
        ))}
        {lines.moreDirs > 0 && <li>{tp("ask.unsaved_dirs_more", lines.moreDirs)}</li>}
        {lines.files > 0 && <li>{tp("ask.unsaved_files", lines.files)}</li>}
        {lines.roots.map((root) => (
          <li key={root}>{t("ask.unsaved_root", { path: root })}</li>
        ))}
      </ul>
      <div className="ask-unsaved-note">{t("ask.unsaved_note")}</div>
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
    case "mcpServer":
      return t("ask.scope.mcp_server", { server: scope.server });
    case "directory":
      return scope.path === ""
        ? t("ask.scope.directory_root")
        : t("ask.scope.directory", { path: scope.path });
  }
}
