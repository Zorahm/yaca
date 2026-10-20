/**
 * Settings → Connection: where this window's work happens.
 *
 * On a computer it can be this computer or a server; on a phone only a server,
 * so the only thing to do there is to leave it for another one.
 */

import { forgetRemote, isMobile, isRemote, savedRemote, setLinkMode } from "../../core/link";
import { t } from "../../i18n";
import { Row } from "../settings/Row";

export function ConnectionSection() {
  const remote = isRemote();
  const saved = savedRemote();

  const toServer = () => {
    setLinkMode("remote");
    window.location.reload();
  };
  const toLocal = () => {
    setLinkMode("local");
    window.location.reload();
  };
  const leave = () => {
    forgetRemote();
    if (!isMobile) setLinkMode("local");
    window.location.reload();
  };

  return (
    <>
      <h1 className="set-title">{t("link.settings_title")}</h1>
      <div className="set-rows">
        <Row
          name={remote ? t("link.settings_server", { name: saved?.label ?? "" }) : t("link.settings_this_pc")}
          hint={t("link.settings_reload_note")}
        >
          <div className="btn-row">
            {!remote && (
              <button type="button" className="btn btn-small" onClick={toServer}>
                {t("link.settings_to_server")}
              </button>
            )}
            {remote && !isMobile && (
              <button type="button" className="btn btn-small" onClick={toLocal}>
                {t("link.settings_to_local")}
              </button>
            )}
            {remote && (
              <button type="button" className="btn btn-small" onClick={leave}>
                {t("link.settings_disconnect")}
              </button>
            )}
          </div>
        </Row>
      </div>
    </>
  );
}
