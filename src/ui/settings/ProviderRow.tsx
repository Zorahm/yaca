import { ArrowClockwise, CaretRight, DotsThree, Key, SpinnerGap, Trash } from "@phosphor-icons/react";
import { DropdownMenu } from "radix-ui";
import { useState } from "react";

import type { ProviderSpec } from "../../core/client";
import type { ProviderPoll } from "../../core/bindings/ProviderPoll";
import { capitalize, t, tp, type Key as TranslationKey } from "../../i18n";
import { ProviderMark } from "../brand";
import { dateTimeOf } from "../format";
import { isPreset, type KeyState } from "./provider";
import { pollAge, pollErrorLabel, providerAddress, type ProviderStatus } from "./providerStatus";

const SOURCE_LABEL: Record<KeyState["kind"], TranslationKey> = {
  keychain: "prov.key_source_keychain", environment: "prov.key_source_environment",
  none: "prov.key_source_none", not_needed: "prov.keyless_short",
  loading: "prov.key_checking", unknown: "prov.key_unavailable",
};

export function ProviderRow({ spec, source, poll, status, pollError, now, muted, current, locked, onAsk, onEdit, onDelete }: {
  spec: ProviderSpec;
  source: KeyState;
  poll?: ProviderPoll;
  status: ProviderStatus;
  pollError?: string;
  now: number;
  muted: boolean;
  current: boolean;
  locked: boolean;
  onAsk: () => void;
  onEdit: (focusKey?: boolean) => void;
  onDelete: () => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const address = providerAddress(spec.baseUrl);
  const error = pollError ?? poll?.error ?? undefined;
  const busy = status === "asking" || status === "loading";
  const statusText = status === "ready" ? tp("prov.models_count", spec.models.length)
    : status === "error" ? pollErrorLabel(error ?? "")
    : status === "empty" ? t("prov.status_empty")
    : status === "missing_key" ? t("prov.status_missing_key")
    : status === "asking" ? t("prov.models_asking")
    : status === "loading" ? t("prov.key_checking")
    : status === "unknown" ? t("prov.key_unavailable") : capitalize(t("prov.unasked"));
  const actionLabel = status === "missing_key" ? t("prov.add_key")
    : status === "unknown" ? t("prov.edit_row")
    : status === "error" || status === "empty" ? t("prov.retry")
    : status === "unasked" ? t("prov.models_ask") : t("prov.refresh");
  const textAction = ["missing_key", "unknown", "error", "empty", "unasked"].includes(status);

  return <div className={`provider-row${muted ? " provider-row-muted" : ""}`} data-status={status} aria-busy={status === "asking"}>
    <button type="button" className="provider-open" disabled={locked} onClick={() => onEdit()}
      title={[spec.baseUrl, poll ? dateTimeOf(poll.checkedAt) : ""].filter(Boolean).join(" · ")}
      aria-label={`${t("prov.edit_row")}: ${spec.displayName}`} />
    <div className="provider-identity">
      <span className="provider-avatar" aria-hidden="true">
        <ProviderMark id={spec.id} size={20} fallbackName={spec.displayName} />
      </span>
      <div className="provider-description">
        <div className="provider-name-line">
          <span className="provider-name">{spec.displayName}</span>
          <span className="provider-chip provider-protocol">{t(spec.protocol === "anthropic" ? "prov.protocol_anthropic" : "prov.protocol_openai")}</span>
          {address.local && <span className="provider-chip">{t("prov.local")}</span>}
          {!isPreset(spec.id) && <span className="provider-chip">{t("prov.custom")}</span>}
          {current && <span className="provider-chip provider-current">{t("prov.current_chat")}</span>}
        </div>
        <div className="provider-meta">
          <span className="provider-host" title={spec.baseUrl}>{address.host}</span>
          <span aria-hidden="true">·</span>
          <span>{t(SOURCE_LABEL[source.kind])}</span>
        </div>
      </div>
    </div>
    <div className="provider-result">
      <span className="provider-status" aria-live="polite" aria-atomic="true">
        {busy ? <SpinnerGap size={13} className="prov-spin" aria-hidden="true" />
          : status === "missing_key" ? <Key size={13} aria-hidden="true" />
          : <span className="provider-status-dot" aria-hidden="true" />}
        {statusText}
      </span>
      {poll ? <time className="provider-time" dateTime={new Date(poll.checkedAt).toISOString()} title={dateTimeOf(poll.checkedAt)}>
        {pollAge(poll.checkedAt, now)}
      </time> : status === "ready" && <span className="provider-time">{t("prov.checked_unknown")}</span>}
    </div>
    <div className="provider-action">
      <button type="button" className={`btn btn-small${status === "missing_key" ? " provider-add-key" : ""}`}
        disabled={busy || locked} title={actionLabel} aria-label={`${actionLabel}: ${spec.displayName}`}
        onClick={() => status === "missing_key" || status === "unknown" ? onEdit(status === "missing_key") : onAsk()}>
        {!textAction && <ArrowClockwise size={15} aria-hidden="true" />}
        {textAction && actionLabel}
      </button>
    </div>
    <div className="provider-menu-slot">
      {!isPreset(spec.id) && <DropdownMenu.Root open={menuOpen} onOpenChange={(open) => { setMenuOpen(open); if (!open) setConfirming(false); }}>
        <DropdownMenu.Trigger asChild>
          <button type="button" className="btn btn-row" disabled={locked} aria-label={`${t("prov.actions")}: ${spec.displayName}`}>
            <DotsThree size={18} weight="bold" />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content className="pop provider-menu" align="end" sideOffset={5}
            onEscapeKeyDown={(event) => event.stopPropagation()}>
            {confirming && <div className="provider-delete-note">{t("prov.delete_note", { name: spec.displayName })}</div>}
            <DropdownMenu.Item className="pop-item pop-item-danger" onSelect={(event) => {
              if (confirming) onDelete();
              else { event.preventDefault(); setConfirming(true); }
            }}><Trash size={14} />{t(confirming ? "prov.delete_confirm" : "prov.delete")}</DropdownMenu.Item>
            {confirming && <DropdownMenu.Item className="pop-item" onSelect={() => setMenuOpen(false)}>{t("prov.form_cancel")}</DropdownMenu.Item>}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>}
    </div>
    <CaretRight size={14} className="provider-chevron" aria-hidden="true" />
    {(error || address.local) && <div className={`provider-detail${error ? " provider-detail-error" : ""}`}>
      {error && <span title={error}>{error}</span>}
      {address.local && <span>{t("prov.local_hint")}</span>}
    </div>}
  </div>;
}
