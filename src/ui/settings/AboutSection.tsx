/**
 * About the application: the mark, the version, what it is, and whether a
 * newer release exists.
 *
 * Both items of the Help menu lead here (`core/menu`). "Check for updates"
 * lands on the same button as the one in the row, so there is one place where
 * the state is written (§0).
 *
 * The answer comes from the shell (`ui/update`), which also asks by itself in
 * the background when the setting is on. A found release is shown with its
 * changelog — the text of the release on GitHub — and two links; nothing is
 * downloaded or installed by the app.
 */

import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";

import type { Preferences, ReleaseInfo } from "../../core/client";
import { appVersion } from "../../core/menu";
import { t } from "../../i18n";
import { YacaMark } from "../Logo";
import { MarkdownDoc } from "../MarkdownDoc";
import { updateErrorKey, useUpdate } from "../update";
import { Row, Switch } from "./Row";

export function AboutSection({
  prefs,
  onPatch,
}: {
  prefs: Preferences;
  onPatch: (change: Partial<Preferences>) => void;
}) {
  // `null` until the shell answers; a version that was guessed meanwhile
  // would be the one thing on this screen that must not be a guess.
  const [version, setVersion] = useState<string | null>(null);
  const update = useUpdate();

  useEffect(() => {
    let live = true;
    appVersion().then(
      (answer) => {
        if (live) setVersion(answer);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  const release = update.check?.latest ?? null;

  let status: string | null = null;
  if (update.checking) status = t("about.update_checking");
  else if (update.error !== null) status = t(updateErrorKey(update.error));
  else if (release !== null) status = t("about.update_available", { version: release.version });
  else if (update.check !== null) status = t("about.update_latest");

  return (
    <>
      <div className="about-head">
        <YacaMark size={52} />
        <div>
          <h1 className="about-name">Yaca</h1>
          <p className="about-version">
            {version === null ? " " : t("about.version", { version })}
          </p>
        </div>
      </div>

      <p className="about-text">{t("about.what")}</p>
      <p className="about-text">{t("about.how")}</p>

      <div className="set-rows">
        <Row name={t("about.updates")} hint={t("about.updates_hint")}>
          <div className="about-update-control">
            {status !== null && (
              <span
                className="about-value"
                role="status"
                data-state={update.error !== null ? "error" : release !== null ? "new" : "ok"}
              >
                {status}
              </span>
            )}
            <button
              type="button"
              className="btn btn-small"
              disabled={update.checking}
              onClick={update.run}
            >
              {update.checking ? t("about.update_checking") : t("about.update_check")}
            </button>
          </div>
        </Row>
        <Row name={t("about.update_auto")} hint={t("about.update_auto_hint")}>
          <Switch
            on={prefs.checkUpdates}
            label={t("about.update_auto")}
            onChange={(checkUpdates) => onPatch({ checkUpdates })}
          />
        </Row>
        <Row name={t("about.license")}>
          <span className="about-value">MIT</span>
        </Row>
      </div>

      {release !== null && <ReleaseCard release={release} />}
    </>
  );
}

function ReleaseCard({ release }: { release: ReleaseInfo }) {
  const open = (url: string) => void openUrl(url).catch(() => undefined);
  const published = release.publishedAt === null ? null : new Date(release.publishedAt);
  const date =
    published !== null && !Number.isNaN(published.getTime())
      ? published.toLocaleDateString()
      : null;

  return (
    <section className="about-release" aria-label={release.name}>
      <header className="about-release-head">
        <h2 className="about-release-name">{release.name}</h2>
        <span className="about-release-meta">
          {release.version}
          {release.prerelease && ` · ${t("about.update_prerelease")}`}
          {date !== null && ` · ${date}`}
        </span>
      </header>

      <h3 className="about-release-label">{t("about.update_changes")}</h3>
      <div className="about-release-body">
        {release.changelog === "" ? (
          <p className="about-value">{t("about.update_no_changes")}</p>
        ) : (
          <MarkdownDoc text={release.changelog} />
        )}
      </div>

      <div className="about-release-actions">
        <button
          type="button"
          className="btn btn-accent btn-small"
          onClick={() => open(release.downloadUrl ?? release.url)}
        >
          {t("about.update_download")}
        </button>
        {release.downloadUrl !== null && (
          <button type="button" className="btn btn-small" onClick={() => open(release.url)}>
            {t("about.update_page")}
          </button>
        )}
      </div>
    </section>
  );
}
