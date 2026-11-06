import type { Preferences } from "../../core/client";
import { t } from "../../i18n";

export function AppearancePreview({ prefs }: { prefs: Preferences }) {
  const lines = ["export const greeting = () => {", '-  return "Hello";', '+  return "Hello, world!";', "};"];
  return <figure className="set-preview">
    <figcaption>{t("settings.preview")}</figcaption>
    <div className="set-preview-window">
      <div className="set-preview-bar"><span>{t("settings.preview_chat")}</span><span>example.ts</span></div>
      <p className="set-preview-message">{t("settings.preview_message")}</p>
      <div className="set-preview-diff" style={{ lineHeight: prefs.compact ? "20px" : "22px" }}>
        {lines.map((line, index) => <div key={line} data-change={line[0] === "+" ? "add" : line[0] === "-" ? "del" : undefined}>
          {prefs.lineNumbers && <span className="set-preview-number">{index + 1}</span>}
          <code>{line}</code>
        </div>)}
      </div>
      {prefs.notifications && <div className="set-preview-toast">{t("settings.preview_toast")}</div>}
    </div>
  </figure>;
}
