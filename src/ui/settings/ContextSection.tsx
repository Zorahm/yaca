import { useEffect, useState } from "react";
import { projectCommands, projectSkills, projectInstructionsPreview, openProjectInstructions, type SkillReport, type SlashCommand } from "../../core/client";
import { t, type Key } from "../../i18n";
const SCOPE: Record<SkillReport["skills"][number]["scope"], Key> = {
  project: "proj.skills_project",
  user: "proj.skills_user",
  plugin: "proj.skills_plugin",
};


export function ContextSection({ project }: { project: string }) {
  const [instructions, setInstructions] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [commands, setCommands] = useState<SlashCommand[]>([]);
  const [skills, setSkills] = useState<SkillReport>({ skills: [], diagnostics: [] });
  useEffect(() => {
    let live = true;
    setInstructions(null); setText(""); setError(null);
    void projectInstructionsPreview(project).then(
      (file) => { if (live) { setInstructions(file?.[0] ?? null); setText(file?.[1] ?? ""); } },
      (e: unknown) => live && setError(String(e)),
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


  return (<>
    <div className="set-head"><div><h1 className="set-title">{t("settings.context")}</h1><p className="set-lede">{t("proj.skills_hint")}</p></div></div>
    {error && <div className="set-error" role="alert">{error}</div>}
      <div className="set-subhead">
        <div>
          <span className="set-subname">{t("proj.instructions")}</span>
          <span className="set-subhint">
            {instructions === null ? t("proj.instructions_none") : t("proj.instructions_found", { path: instructions })}
          </span>
        </div>
      </div>


    {instructions && <>
      <button type="button" className="btn btn-small" onClick={() => void openProjectInstructions(project).catch((e: unknown) => setError(String(e)))}>{t("settings.open_editor")}</button>
      <pre className="set-instructions-preview" aria-label={t("proj.instructions")}>{text}</pre>
    </>}
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
