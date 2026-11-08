import { useEffect, useState } from "react";
import { browser, type BrowserState, type SessionId } from "../../core/client";
import { t } from "../../i18n";

export function BrowserAccess({ session, state, options, run }: {
  session: SessionId;
  state: BrowserState | null;
  options: boolean;
  run: (work: Promise<unknown>) => Promise<unknown>;
}) {
  const request = state?.request;
  const control = (mode: "pause" | "take" | "resume") => void run(browser.control(session, mode));
  return (
    <>
      {(state?.busy || state?.paused) && (
        <div className="browser-agent">
          <div className="browser-agent-label" role="status">
            <span className="browser-agent-dot" />
            <span>{t(state.paused
              ? state.busy ? "browser.pausing" : state.taken ? "browser.you_control" : "browser.paused"
              : "browser.busy")}</span>
          </div>
          {state.paused ? (
            <button type="button" className="btn" onClick={() => control("resume")}>{t("browser.resume")}</button>
          ) : (
            <>
              <button type="button" className="btn" onClick={() => control("pause")}>{t("browser.pause")}</button>
              <button type="button" className="btn" onClick={() => control("take")}>{t("browser.take")}</button>
            </>
          )}
        </div>
      )}
      {request && (
        <section className="browser-request" aria-label={t("browser.permission")}>
          <strong>{t(request.submission ? "browser.confirm_submit" : "browser.confirm_open")}</strong>
          <code>{request.method} {request.origin}</code>
          <div>
            <button type="button" className="btn" onClick={() => void run(browser.decide(session, request.id, false))}>
              {t("browser.deny")}
            </button>
            <button type="button" className="btn btn-accent" onClick={() => void run(browser.decide(session, request.id, true))}>
              {t("browser.allow_once")}
            </button>
          </div>
        </section>
      )}
      {options && <AllowedOrigins />}
    </>
  );
}

function AllowedOrigins() {
  const [draft, setDraft] = useState<string | null>(null);
  const [saved, setSaved] = useState("");
  const [error, setError] = useState(false);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    let active = true;
    void browser.origins().then((origins) => {
      if (active) {
        setSaved(origins.join("\n"));
        setDraft(origins.join("\n"));
      }
    }, () => {
      if (active) setError(true);
    });
    return () => { active = false; };
  }, []);

  return (
    <form className="browser-origins" onSubmit={(event) => {
      event.preventDefault();
      if (draft === null) return;
      setSaving(true);
      setError(false);
      void browser.origins(draft).then((origins) => {
        setSaved(origins.join("\n"));
        setDraft(origins.join("\n"));
      }, () => setError(true)).finally(() => setSaving(false));
    }}>
      <label htmlFor="browser-origins">{t("browser.origins")}</label>
      <textarea id="browser-origins" rows={3} value={draft ?? ""} spellCheck={false}
        disabled={draft === null || saving} placeholder="https://example.org"
        onChange={(event) => setDraft(event.target.value)} />
      <small>{t("browser.origins_hint")}</small>
      {error && <small className="browser-origin-error" role="alert">{t("browser.origins_error")}</small>}
      <button type="submit" className="btn" disabled={draft === null || draft === saved || saving}>
        {t("browser.save")}
      </button>
    </form>
  );
}
