/**
 * Проект (§10): файл инструкций, исключения теневого хранилища, свои команды.
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
  shadowExcludedDirs,
  type SlashCommand,
  type Settings,
} from "../../core/client";
import { t } from "../../i18n";

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
          <p className="set-lede">{project}</p>
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
    </>
  );
}
