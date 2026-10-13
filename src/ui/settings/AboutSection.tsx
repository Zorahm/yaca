/**
 * About the application: the mark, the version, what it is.
 *
 * Both items of the Help menu lead here (`core/menu`). "Check for updates"
 * does so because there is nothing to check against yet: releases are not
 * published, and the row below says that in words. A menu item that did
 * nothing when pressed would be the interface lying about its state (§0);
 * one that opens the place where the state is written does not.
 */

import { useEffect, useState } from "react";

import { appVersion } from "../../core/menu";
import { t } from "../../i18n";
import { YacaMark } from "../Logo";
import { Row } from "./Row";

export function AboutSection() {
  // `null` until the shell answers; a version that was guessed meanwhile
  // would be the one thing on this screen that must not be a guess.
  const [version, setVersion] = useState<string | null>(null);

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

  return (
    <>
      <div className="about-head">
        <YacaMark size={52} />
        <div>
          <h1 className="about-name">Yaca</h1>
          <p className="about-version">
            {version === null ? " " : t("about.version", { version })}
          </p>
        </div>
      </div>

      <p className="about-text">{t("about.what")}</p>
      <p className="about-text">{t("about.how")}</p>

      <div className="set-rows">
        <Row name={t("about.updates")} hint={t("about.updates_hint")}>
          <span className="about-value">{t("about.updates_none")}</span>
        </Row>
        <Row name={t("about.license")}>
          <span className="about-value">MIT</span>
        </Row>
      </div>
    </>
  );
}
