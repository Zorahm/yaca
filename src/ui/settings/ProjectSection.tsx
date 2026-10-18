/**
 * Проект (§10): файл инструкций, исключения теневого хранилища, свои команды,
 * навыки.
 *
 * Инструкции и команды показаны, но не правятся: это файлы в дереве, и место,
 * где их меняют, — редактор, а не окно настроек. Окно говорит, какой файл в
 * силе, — а спорит оно только за то, чего в дереве нет.
 *
 * Исключения снимка §5.8 добавляются к встроенным, а не заменяют их: проект,
 * выкинувший `.git` и `.yaca`, снимал бы снимки со своих снимков. Поэтому они
 * и показаны — чипами без крестика, чтобы «а почему их тут нет» не приходилось
 * выяснять чтением исходников.
 */

import { Plus, X } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import {
  projectCommands,
  projectInstructions,
  projectSkills,
  shadowExcludedDirs,
  type SkillReport,
  type SlashCommand,
  type Settings,
} from "../../core/client";
import { t, type Key } from "../../i18n";
import { plainPath } from "../format";

const SCOPE: Record<SkillReport["skills"][number]["scope"], Key> = {
  project: "proj.skills_project",
  user: "proj.skills_user",
  plugin: "proj.skills_plugin",
};

export function ProjectSection({
  project,
  settings,
  onPatch,
}: {
  project: string;
  settings: Settings;
  onPatch: (change: (settings: Settings) => Settings) => void;
}) {
  const [instructions, setInstructions] = useState<string | null>(null);
  const [commands, setCommands] = useState<SlashCommand[]>([]);
  const [skills, setSkills] = useState<SkillReport>({ skills: [], diagnostics: [] });
  const [fixed, setFixed] = useState<string[]>([]);
  const [adding, setAdding] = useState("");

  // Встроенный список спрашивается у ядра один раз: он не меняется, пока
  // приложение живёт.
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

  useEffect(() => {
    let live = true;
    void projectInstructions(project).then(
      (path) => live && setInstructions(path),
      () => live && setInstructions(null),
    );
    void projectCommands(project).then(
      (list) => live && setCommands(list),
      () => live && setCommands([]),
    );
    void projectSkills(project).then(
      (report) => live && setSkills(report),
      () => live && setSkills({ skills: [], diagnostics: [] }),
    );
    return () => {
      live = false;
    };
  }, [project]);

  const exclude = settings.shadow.exclude;

  const add = () => {
    const name = adding.trim();
    if (name === "" || exclude.includes(name)) return setAdding("");
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
          <h1 className="set-title">{t("proj.title")}</h1>
          <p className="set-lede">{plainPath(project)}</p>
        </div>
      </div>

      <div className="set-subhead">
        <div>
          <span className="set-subname">{t("proj.instructions")}</span>
          <span className="set-subhint">
            {instructions === null ? t("proj.instructions_none") : t("proj.instructions_found", { path: instructions })}
          </span>
        </div>
      </div>

      <div className="set-subhead set-subhead-sep">
        <div>
          <span className="set-subname">{t("proj.exclude")}</span>
          <span className="set-subhint">{t("proj.exclude_hint")}</span>
        </div>
      </div>

      <div className="chips">
        {fixed.map((name) => (
          <span key={name} className="chip chip-fixed" title={t("proj.exclude_fixed")}>
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

      <div className="set-subhead set-subhead-sep">
        <div>
          <span className="set-subname">{t("proj.commands")}</span>
          <span className="set-subhint">{t("proj.commands_hint")}</span>
        </div>
      </div>

      <div className="set-list">
        {commands.length === 0 && <div className="set-empty">{t("proj.commands_none")}</div>}
        {commands.map((command) => (
          <div key={command.name} className="rule">
            <span className="rule-value">/{command.name}</span>
            <span className="rule-reason">{command.description}</span>
          </div>
        ))}
      </div>

      <div className="set-subhead set-subhead-sep">
        <div>
          <span className="set-subname">{t("proj.skills")}</span>
          <span className="set-subhint">{t("proj.skills_hint")}</span>
        </div>
      </div>

      {skills.skills.length === 0 && <div className="set-empty">{t("proj.skills_none")}</div>}
      <div className="prov-list">
        {skills.skills.map((skill) => (
          <div key={skill.name} className="prov plug" title={skill.path}>
            <div className="prov-main">
              <span className="prov-name skill-name">{skill.name}</span>
              <span className="plug-desc">{skill.description}</span>
              {!skill.modelInvocable && <span className="plug-note">{t("proj.skills_slash_only")}</span>}
            </div>
            <span className="prov-proto">{t(SCOPE[skill.scope])}</span>
          </div>
        ))}
      </div>

      {skills.diagnostics.length > 0 && (
        <>
          <div className="set-subhead set-subhead-sep">
            <div>
              <span className="set-subname">{t("proj.skills_problems")}</span>
            </div>
          </div>
          <div className="set-list">
            {skills.diagnostics.map((d, at) => (
              <div key={`${d.path}-${at}`} className="diag" data-skipped={d.skipped || undefined}>
                <span className="diag-message">
                  <span className="diag-kind">
                    {d.skipped ? t("proj.skills_skipped") : t("proj.skills_warning")}
                  </span>{" "}
                  {d.message}
                </span>
                <span className="diag-path">{d.path}</span>
              </div>
            ))}
          </div>
        </>
      )}
    </>
  );
}
