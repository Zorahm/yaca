/**
 * Окно настроек §10.
 *
 * Навигация разрезана надвое — приложение и проект. §10 перечисляет разделы
 * одним списком, но в нём смешаны два уровня правды: провайдеры, внешний вид и
 * уведомления общие для всего, а модели, режим, правила и исключения снимков —
 * этого проекта. Плоским списком человек менял бы «Режим» и не знал, где
 * именно, поэтому у проектной группы стоит чип папки, и он же переключатель.
 *
 * Разделов семь, а не девять. MCP и языковых серверов нет, потому что за ними
 * нет ничего в ядре, а раздел, который ничего не меняет, — это интерфейс,
 * врущий про состояние (§0).
 *
 * Настройки применяются сразу, без кнопки «Сохранить»: она разрешила бы окну
 * показывать одно, а диску держать другое. Исключение — описание провайдера:
 * это документ, и недописанный адрес на диск попадать не должен.
 */

import {
  Bell,
  Cube,
  FolderSimple,
  Palette,
  Plugs,
  ShieldCheck,
  Wrench,
} from "@phosphor-icons/react";
import { useCallback, useEffect, useState, type ReactNode } from "react";

import {
  models as fetchModels,
  projects as listProjects,
  setPreferences,
  type ModelInfo,
  type Preferences,
} from "./core/client";
import { t, type Key } from "./i18n";
import { usePreferences } from "./ui/prefs";
import { AppearanceSection } from "./ui/settings/AppearanceSection";
import { ModelsSection } from "./ui/settings/ModelsSection";
import { NoticesSection } from "./ui/settings/NoticesSection";
import { ProjectSection } from "./ui/settings/ProjectSection";
import { ProvidersSection } from "./ui/settings/ProvidersSection";
import { RulesSection } from "./ui/settings/RulesSection";
import { ToolsSection } from "./ui/settings/ToolsSection";
import { useProjectSettings } from "./ui/settings/useProjectSettings";

import "./ui/fonts";
import "./ui/tokens.css";
import "./ui/app.css";
import "./ui/settings/settings.css";

type Section =
  | "providers"
  | "tools"
  | "appearance"
  | "notices"
  | "models"
  | "rules"
  | "project";

type Item = { id: Section; name: Key; icon: ReactNode };

const APP_SECTIONS: Item[] = [
  { id: "providers", name: "prov.title", icon: <Plugs size={15} /> },
  { id: "tools", name: "tools.title", icon: <Wrench size={15} /> },
  { id: "appearance", name: "look.title", icon: <Palette size={15} /> },
  { id: "notices", name: "notices.title", icon: <Bell size={15} /> },
];

const PROJECT_SECTIONS: Item[] = [
  { id: "models", name: "models.title", icon: <Cube size={15} /> },
  { id: "rules", name: "rules.title", icon: <ShieldCheck size={15} /> },
  { id: "project", name: "proj.title", icon: <FolderSimple size={15} /> },
];

function isProjectSection(section: Section): boolean {
  return PROJECT_SECTIONS.some((item) => item.id === section);
}

export default function Settings({ prefs: initial }: { prefs: Preferences }) {
  const prefs = usePreferences(initial);
  const [section, setSection] = useState<Section>("providers");
  const [projects, setProjects] = useState<string[]>([]);
  const [project, setProject] = useState<string | null>(null);
  const [models, setModels] = useState<ModelInfo[]>([]);

  useEffect(() => {
    // Свежий первым — тот же порядок, в котором главное окно выбирает проект
    // по умолчанию, так что окно настроек открывается на том же самом.
    void listProjects().then((list) => {
      setProjects(list);
      setProject((now) => now ?? list[0] ?? null);
    });
  }, []);

  const refreshModels = useCallback(() => {
    void fetchModels().then(setModels, () => setModels([]));
  }, []);

  useEffect(refreshModels, [refreshModels]);

  const patchPrefs = useCallback(
    (change: Partial<Preferences>) => {
      void setPreferences({ ...prefs, ...change }).catch((e: unknown) =>
        console.error("не записать настройки приложения", e),
      );
    },
    [prefs],
  );

  const projectSettings = useProjectSettings(project);

  // Раздел проекта без проекта показать нечем: список пуст, пока ни одной
  // папки не открывали.
  const shown = project === null && isProjectSection(section) ? "providers" : section;

  return (
    <div className="settings">
      <nav className="set-nav" aria-label={t("set.nav")}>
        <div className="set-group">{t("set.group_app")}</div>
        {APP_SECTIONS.map((item) => (
          <NavItem key={item.id} item={item} active={shown} onPick={setSection} />
        ))}

        {project !== null && (
          <>
            <div className="set-group">{t("set.group_project")}</div>
            {/* Чип проекта — он же переключатель: папок в списке несколько, и
                раздел обязан сказать, какую из них он правит. */}
            <select
              className="set-project"
              aria-label={t("set.project")}
              value={project}
              onChange={(e) => setProject(e.target.value)}
            >
              {projects.map((path) => (
                <option key={path} value={path}>
                  {path}
                </option>
              ))}
            </select>
            {PROJECT_SECTIONS.map((item) => (
              <NavItem key={item.id} item={item} active={shown} onPick={setSection} />
            ))}
          </>
        )}
      </nav>

      <main className="set-pane">
        <div className="set-scroll">
          {shown === "providers" && <ProvidersSection onChanged={refreshModels} />}
          {shown === "tools" && <ToolsSection prefs={prefs} onPatch={patchPrefs} />}
          {shown === "appearance" && <AppearanceSection prefs={prefs} onPatch={patchPrefs} />}
          {shown === "notices" && <NoticesSection prefs={prefs} onPatch={patchPrefs} />}

          {isProjectSection(shown) && project !== null && (
            <>
              {projectSettings.error !== null && (
                <div className="set-error">{projectSettings.error}</div>
              )}
              {projectSettings.settings !== null && (
                <>
                  {shown === "models" && (
                    <ModelsSection
                      settings={projectSettings.settings}
                      models={models}
                      onPatch={projectSettings.patch}
                    />
                  )}
                  {shown === "rules" && (
                    <RulesSection
                      settings={projectSettings.settings}
                      onPatch={projectSettings.patch}
                    />
                  )}
                  {shown === "project" && (
                    <ProjectSection
                      project={project}
                      settings={projectSettings.settings}
                      onPatch={projectSettings.patch}
                    />
                  )}
                </>
              )}
            </>
          )}
        </div>
      </main>
    </div>
  );
}

function NavItem({
  item,
  active,
  onPick,
}: {
  item: Item;
  active: Section;
  onPick: (section: Section) => void;
}) {
  return (
    <button
      type="button"
      className="set-nav-item"
      aria-current={active === item.id ? "page" : undefined}
      onClick={() => onPick(item.id)}
    >
      <span className="set-nav-icon" aria-hidden="true">
        {item.icon}
      </span>
      <span className="set-nav-name">{t(item.name)}</span>
    </button>
  );
}
