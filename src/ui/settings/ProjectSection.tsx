/** Project folders and snapshot exclusions; built-in exclusions stay locked. */

import { LockSimple, Plus, X } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import {
  shadowExcludedDirs,
  type Settings,
} from "../../core/client";
import { t } from "../../i18n";
import { ProjectFolders } from "../projects/ProjectEditor";

export function ProjectSection({
  project,
  entry,
  onEdit,
  settings,
  onPatch,
}: {
  project: string;
  entry: import("../../core/client").ProjectEntry | null;
  onEdit: () => void;
  settings: Settings;
  onPatch: (change: (settings: Settings) => Settings) => void;
}) {
  const [fixed, setFixed] = useState<string[]>([]);
  const [adding, setAdding] = useState("");

  // The built-in list is asked of the core once: it does not change while the app
  // is alive.
  useEffect(() => {
    let live = true;
    void shadowExcludedDirs().then(
      (list) => live && setFixed(list),
      () => live && setFixed([]),
    );
    return () => {
      live = false;
    };
  }, []);

  const exclude = settings.shadow.exclude;

  const add = () => {
    const name = adding.trim();
    if (name === "" || fixed.includes(name) || exclude.includes(name)) return setAdding("");
    onPatch((now) => ({
      ...now,
      shadow: { ...now.shadow, exclude: [...now.shadow.exclude, name] },
    }));
    setAdding("");
  };

  return (
    <>
      <div className="set-head">
        <div>
          <h1 className="set-title">{t("settings.configuration")}</h1>
          <p className="set-lede">{entry?.name}</p>
        </div>
        <button type="button" className="btn btn-small" onClick={onEdit}>{t("project.edit")}</button>
      </div>
      <div className="set-subhead"><span className="set-subname">{t("project.folders")}</span></div>
      <ProjectFolders folders={entry?.folders ?? [project]} />
      <p className="project-hint">{t("project.primary_hint")}</p>
      {entry && entry.folders.length > 1 && <p className="project-hint">{t("project.rollback_hint")}</p>}

      <div className="set-subhead set-subhead-sep">
        <div>
          <span className="set-subname">{t("proj.exclude")}</span>
          <span className="set-subhint">{t("proj.exclude_hint")}</span>
        </div>
      </div>

      <div className="chips">
        {fixed.map((name) => (
          <span key={name} className="chip chip-fixed" title={t("proj.exclude_fixed")}>
            <LockSimple size={11} aria-hidden="true" />
            {name}
          </span>
        ))}
        {exclude.map((name) => (
          <span key={name} className="chip">
            {name}
            <button
              type="button"
              className="btn btn-row"
              aria-label={t("proj.exclude_remove", { name })}
              onClick={() =>
                onPatch((now) => ({
                  ...now,
                  shadow: {
                    ...now.shadow,
                    exclude: now.shadow.exclude.filter((other) => other !== name),
                  },
                }))
              }
            >
              <X size={11} />
            </button>
          </span>
        ))}
      </div>

      <div className="rule-add">
        <input
          className="prov-input prov-input-mono"
          value={adding}
          spellCheck={false}
          aria-label={t("proj.exclude")}
          placeholder={t("proj.exclude_placeholder")}
          onChange={(e) => setAdding(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
        />
        <button
          type="button"
          className="btn btn-small"
          disabled={adding.trim() === ""}
          onClick={add}
        >
          <Plus size={13} /> {t("proj.exclude_add")}
        </button>
      </div>

    </>
  );
}
