/**
 * The model router in the settings window (§10): `routing.json`, edited as
 * the document it is.
 *
 * A text editor rather than a form on purpose. The file is a small language —
 * questions, rules over their answers, a model per rule — and a form for each
 * shape would be a second copy of the core's types to keep in step. What the
 * window adds is what an editor cannot: the file is checked with the core's
 * own rules before it is written, so a typo is refused here, in the words the
 * start would refuse it with, and not by the next chat that fails to open.
 *
 * Like a provider, it is a document: Save and nothing is written until then.
 * What a chat does with it is decided when the chat opens, so a saved router
 * reaches new chats and the open ones go on as they were.
 */

import { useCallback, useEffect, useState } from "react";

import {
  removeRouting,
  ROUTER_PROVIDER_ID,
  routingStatus,
  routingText,
  saveRouting,
  type ModelInfo,
  type RoutingStatus,
} from "../../core/client";
import { DECISION_PROVIDER_ID } from "../../core/provider";
import { t } from "../../i18n";
import { modelKey } from "../modelSearch";
import { Select } from "./Row";
import { configText, emptyPicks, NO_MODEL, type Picks } from "./routingWizard";

/**
 * What an empty file starts from. Ids are placeholders on purpose: this window
 * cannot know which of the person's models is small and which is strong, and a
 * guess written into their file would route conversations somewhere they did
 * not choose. The placeholders fail the check by name until they are replaced.
 */
const TEMPLATE = JSON.stringify(
  {
    name: "auto",
    decisionModel: { provider: "decision", model: "<small model id>", timeoutMs: 3000 },
    default: { provider: "<provider id>", model: "<model id>" },
    signals: [
      {
        name: "needs_reasoning",
        question: {
          type: "noul",
          instructions: "Does answering this request need multi-step reasoning?",
        },
      },
    ],
    decisions: [
      {
        name: "hard",
        priority: 100,
        onUnknown: "noMatch",
        rules: { signal: { name: "needs_reasoning", predicate: { gte: 0.7 } } },
        model: { provider: "<provider id>", model: "<strong model id>" },
      },
    ],
  },
  null,
  2,
);

export function RoutingSection({
  models,
  onChanged,
}: {
  /** Every model the providers listed; the router's own entry is not one to route to. */
  models: ModelInfo[];
  onChanged: () => void;
}) {
  const [status, setStatus] = useState<RoutingStatus | null>(null);
  /** The file as it is on disk; `null` when there is none. */
  const [disk, setDisk] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [picks, setPicks] = useState<Picks>(emptyPicks);

  // The Decision provider's models first: that is the place made for the small
  // model, and the list is long once every provider has answered.
  const routable = models
    .filter((m) => m.providerId !== ROUTER_PROVIDER_ID)
    .sort(
      (a, b) =>
        Number(b.providerId === DECISION_PROVIDER_ID) -
        Number(a.providerId === DECISION_PROVIDER_ID),
    );
  const decisionReady = routable.some((m) => m.providerId === DECISION_PROVIDER_ID);
  const built = configText(routable, picks);

  const load = useCallback(async () => {
    try {
      const [now, text] = await Promise.all([routingStatus(), routingText()]);
      setStatus(now);
      setDisk(text);
      setDraft(text ?? TEMPLATE);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const dirty = draft !== (disk ?? TEMPLATE);

  async function save() {
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      await saveRouting(draft);
      await load();
      setSaved(true);
      onChanged();
    } catch (e) {
      // The core's refusal, as the file would have been refused.
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    setConfirming(false);
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      await removeRouting();
      await load();
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="routing" aria-label={t("routing.title")}>
      <div className="routing-head">
        <h2 className="routing-title">{t("routing.title")}</h2>
        <p className="set-lede">{t("routing.lede")}</p>
      </div>

      {error !== null && <div className="set-error">{error}</div>}

      {status !== null && (
        <p className="set-subhint" data-state={status.state}>
          {status.state === "absent" && t("routing.state_absent")}
          {status.state === "ready" && t("routing.state_ready", { name: status.model.id })}
          {status.state === "broken" && t("routing.state_broken", { error: status.error })}
        </p>
      )}

      <section className="routing-wizard" aria-label={t("routing.wizard_title")}>
        <h2 className="routing-wizard-title">{t("routing.wizard_title")}</h2>
        <p className="set-subhint">{t("routing.wizard_lede")}</p>
        {!decisionReady && <p className="set-subhint">{t("routing.decision_hint")}</p>}
        {routable.length === 0 ? (
          <p className="set-empty">{t("routing.wizard_none")}</p>
        ) : (
          <>
            {(
              [
                ["decider", "routing.wizard_decider"],
                ["fallback", "routing.wizard_default"],
                ["strong", "routing.wizard_strong"],
              ] as const
            ).map(([field, label]) => (
              <div className="routing-pick" key={field}>
                <span className="prov-label">{t(label)}</span>
                <Select
                  mono
                  align="start"
                  label={t(label)}
                  value={picks[field]}
                  options={[
                    { value: NO_MODEL, label: t("routing.wizard_pick") },
                    ...routable.map((m) => ({
                      value: modelKey(m),
                      label: `${m.displayName} — ${m.providerId}`,
                    })),
                  ]}
                  onChange={(value) => setPicks((now) => ({ ...now, [field]: value }))}
                />
              </div>
            ))}
            <button
              type="button"
              className="btn btn-small"
              disabled={built === null}
              onClick={() => {
                if (built === null) return;
                setDraft(built);
                setSaved(false);
                setError(null);
              }}
            >
              {t("routing.wizard_build")}
            </button>
          </>
        )}
      </section>

      <textarea
        className="prov-input prov-input-mono routing-editor"
        aria-label={t("routing.editor")}
        spellCheck={false}
        value={draft}
        disabled={status === null}
        onChange={(event) => {
          setDraft(event.target.value);
          setSaved(false);
        }}
      />

      <div className="routing-actions">
        <button
          type="button"
          className="btn btn-small btn-accent"
          disabled={busy || !dirty}
          onClick={() => void save()}
        >
          {t("routing.save")}
        </button>
        {disk !== null &&
          (confirming ? (
            <button
              type="button"
              className="btn btn-small btn-danger"
              disabled={busy}
              onClick={() => void remove()}
            >
              {t("mcp.delete_confirm")}
            </button>
          ) : (
            <button
              type="button"
              className="btn btn-small"
              disabled={busy}
              onClick={() => setConfirming(true)}
            >
              {t("routing.remove")}
            </button>
          ))}
        {saved && <span className="set-subhint">{t("routing.saved")}</span>}
      </div>

      <p className="set-foot-note">{t("routing.next_chat")}</p>
    </section>
  );
}
