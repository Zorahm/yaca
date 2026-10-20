/**
 * The screen a window shows before it has a server: the address, the token,
 * and — on a computer, which has a host of its own — the way back to it.
 */

import { useState } from "react";

import {
  forgetRemote,
  isMobile,
  parseConnectionLink,
  saveRemote,
  setLinkMode,
  socketAddress,
  savedRemote,
} from "../../core/link";
import { link, LinkError, type LinkFault } from "../../core/remote";
import { t, type Key } from "../../i18n";
import { YacaMark } from "../Logo";

import "../fonts";
import "../tokens.css";
import "../app.css";
import "./link.css";

const FAULT_TEXT: Record<LinkFault, Key> = {
  unreachable: "link.err_unreachable",
  unauthorized: "link.err_unauthorized",
  bad_address: "link.err_bad_address",
  replaced: "link.err_replaced",
  revoked: "link.err_revoked",
};

export function ConnectScreen({
  first,
  onConnected,
}: {
  /** Why the saved server could not be reached, if there was one. */
  first: LinkFault | null;
  onConnected: () => void;
}) {
  const saved = savedRemote();
  const [address, setAddress] = useState(saved?.label ?? "");
  const [token, setToken] = useState(saved?.token ?? "");
  const [busy, setBusy] = useState(false);
  const [fault, setFault] = useState<LinkFault | null>(first);

  const connect = async () => {
    const url = socketAddress(address);
    if (url === null) {
      setFault("bad_address");
      return;
    }
    setBusy(true);
    setFault(null);
    try {
      await link.connect({ url, token: token.trim(), label: address.trim() });
      saveRemote({ url, token: token.trim(), label: address.trim() });
      onConnected();
    } catch (e) {
      setFault(e instanceof LinkError ? e.fault : "unreachable");
      setBusy(false);
    }
  };

  const useThisComputer = () => {
    forgetRemote();
    setLinkMode("local");
    window.location.reload();
  };

  return (
    <main className="connect">
      <form
        className="connect-card"
        onSubmit={(e) => {
          e.preventDefault();
          void connect();
        }}
      >
        <YacaMark size={44} />
        <h1 className="connect-title">{t("link.title")}</h1>
        <p className="connect-intro">{t("link.intro")}</p>

        <label className="connect-field">
          <span>{t("link.address")}</span>
          <input
            className="prov-input prov-input-mono"
            value={address}
            placeholder={t("link.address_hint")}
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            inputMode="url"
            onChange={(e) => {
              // The installer prints one line with both; pasting it fills both fields.
              const pasted = parseConnectionLink(e.target.value);
              if (pasted !== null) {
                setAddress(pasted.address);
                setToken(pasted.token);
              } else setAddress(e.target.value);
            }}
          />
        </label>
        <label className="connect-field">
          <span>{t("link.token")}</span>
          <input
            className="prov-input prov-input-mono"
            type="password"
            value={token}
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            onChange={(e) => setToken(e.target.value)}
          />
          <small>{t("link.token_hint")}</small>
        </label>

        {fault !== null && (
          <p className="connect-error" role="alert">
            {t(FAULT_TEXT[fault])}
          </p>
        )}

        <div className="link-row">
          <button
            type="submit"
            className="btn btn-accent"
            disabled={busy || address.trim() === "" || token.trim() === ""}
          >
            {busy ? t("link.connecting") : t("link.connect")}
          </button>
          {!isMobile && (
            <button type="button" className="btn" onClick={useThisComputer}>
              {t("link.local")}
            </button>
          )}
        </div>
      </form>
    </main>
  );
}
