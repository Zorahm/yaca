/**
 * Name–value rows for the provider's own headers (§10).
 *
 * Presentation only; the draft lives in `useProviderForm`. The section exists
 * so the difference between "a header whose value carries `{key}`" and "the
 * key itself" stays visible: a header value goes into the open
 * `providers.json`, the key never does. A row that mentions the placeholder
 * is therefore marked with it, right where the secret would otherwise hide.
 */

import { X } from "@phosphor-icons/react";

import { t } from "../../i18n";
import { KEY_PLACEHOLDER, type HeaderDraft } from "./provider";

export function HeadersEditor({
  headers,
  onPatch,
  onAdd,
  onRemove,
}: {
  headers: Array<HeaderDraft>;
  onPatch: (index: number, next: Partial<HeaderDraft>) => void;
  onAdd: () => void;
  onRemove: (index: number) => void;
}) {
  return (
    <div className="prov-rows">
      <div className="prov-headers-grid prov-headers-head" aria-hidden="true">
        <span>{t("prov.form_header_name")}</span>
        <span>{t("prov.form_header_value")}</span>
        <span />
        <span />
      </div>

      {headers.map((header, index) => (
        <div className="prov-headers-grid prov-headers-row" key={index}>
          <input
            className="prov-input prov-input-mono"
            value={header.name}
            aria-label={t("prov.form_header_name")}
            spellCheck={false}
            onChange={(e) => onPatch(index, { name: e.target.value })}
          />
          <input
            className="prov-input prov-input-mono"
            value={header.value}
            aria-label={t("prov.form_header_value")}
            spellCheck={false}
            onChange={(e) => onPatch(index, { value: e.target.value })}
          />
          {/* An empty value is a legitimate override: it drops a protocol
              default header (the README Ollama example does exactly that).
              The mark keeps its column so rows stay aligned. */}
          <span className="prov-key-mark">
            {header.value.includes(KEY_PLACEHOLDER) ? KEY_PLACEHOLDER : ""}
          </span>
          <button
            type="button"
            className="btn btn-row btn-danger"
            aria-label={t("prov.form_remove_header")}
            title={t("prov.form_remove_header")}
            onClick={() => onRemove(index)}
          >
            <X size={14} />
          </button>
        </div>
      ))}

      <div>
        <button type="button" className="btn btn-row" onClick={onAdd}>
          {t("prov.form_add_header")}
        </button>
      </div>
    </div>
  );
}
