import { useEffect, useRef, useState, type FormEvent } from "react";
import { t } from "../../i18n";

export function BrowserAddress({ url, disabled, focus, onOpen }: {
  url: string;
  disabled: boolean;
  focus: number;
  onOpen: (text: string) => void;
}) {
  const field = useRef<HTMLInputElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const [draft, setDraft] = useState<string | null>(null);
  const lastFocus = useRef(focus);
  useEffect(() => {
    if (lastFocus.current !== focus) {
      lastFocus.current = focus;
      button.current?.click();
      field.current?.select();
    }
  }, [focus]);

  useEffect(() => {
    field.current?.select();
  }, [draft !== null]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const text = draft?.trim();
    setDraft(null);
    if (text) onOpen(text);
  };

  const parsed = parseUrl(url);
  const local = parsed !== null && (
    /(^|\.)localhost\.?$|^127(?:\.\d{1,3}){3}$|^\[::1\]$|^0\.0\.0\.0$/.test(parsed.hostname)
  );
  const security = local ? t("browser.local") : parsed?.protocol === "http:" ? t("browser.insecure") : null;
  const host = parsed?.host ?? "";
  const tail = parsed === null || !host ? url : parsed.pathname + parsed.search + parsed.hash;

  return (
    <form className="browser-address" onSubmit={submit}>
      {draft === null && security !== null && <span className="browser-security">{security}</span>}
      {draft === null ? (
        <button
          ref={button}
          type="button"
          className="browser-url"
          title={url || t("browser.address")}
          aria-label={url ? `${t("browser.address")}: ${url}` : t("browser.address")}
          aria-keyshortcuts="Control+L"
          disabled={disabled}
          onClick={() => setDraft(url)}
        >
          {host && <strong>{host}</strong>}
          <span>{tail || t("browser.address")}</span>
        </button>
      ) : (
        <input
          ref={field}
          className="browser-input"
          autoFocus
          spellCheck={false}
          disabled={disabled}
          aria-label={t("browser.address")}
          placeholder={t("browser.address")}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => setDraft(null)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.stopPropagation();
              setDraft(null);
              requestAnimationFrame(() => button.current?.focus());
            }
          }}
        />
      )}
    </form>
  );
}

function parseUrl(value: string): URL | null {
  try {
    return new URL(value);
  } catch {
    return null;
  }
}
