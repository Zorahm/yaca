/** Settings apply immediately; project files retain only explicit overrides. */

import {
  ArrowLeft,
  BookOpen,
  Cube,
  FolderSimple,
  Info,
  Palette,
  Plugs,
  PuzzlePiece,
  ShieldCheck,
  Wrench,
} from "@phosphor-icons/react";
import { useCallback, useEffect, useState, type ReactNode } from "react";

import {
  globalMcpServers,
  models as fetchModels,
  projectMcpFile,
  saveGlobalMcpServers,
  setPreferences,
  type ModelInfo,
  type Preferences,
  type ProjectEntry,
} from "./core/client";
import { t, type Key } from "./i18n";
import { McpMark } from "./ui/brand";
import { ProjectEditor } from "./ui/projects/ProjectEditor";
import { usePreferences } from "./ui/prefs";
import { AboutSection } from "./ui/settings/AboutSection";
import { AppearanceSection } from "./ui/settings/AppearanceSection";
import { McpSection, type McpServers } from "./ui/settings/McpSection";
import { ContextSection } from "./ui/settings/ContextSection";
import { useModelDefaults } from "./ui/settings/useModelDefaults";
import { useUpdate } from "./ui/update";
import { ModelChoices, ModelsSection } from "./ui/settings/ModelsSection";
import { PluginsSection } from "./ui/settings/PluginsSection";
import { ProjectSection } from "./ui/settings/ProjectSection";
import { ProvidersSection } from "./ui/settings/ProvidersSection";
import { Segments, Select } from "./ui/settings/Row";
import { RulesSection } from "./ui/settings/RulesSection";
import { ToolsSection } from "./ui/settings/ToolsSection";
import { useProjectSettings } from "./ui/settings/useProjectSettings";
import { useBackClose } from "./ui/back";

import "./ui/fonts";
import "./ui/tokens.css";
import "./ui/app.css";
import "./ui/popmenu.css";
import "./ui/settings/settings.css";
import "./ui/settings/layout.css";

export type Section =
  | "providers"
  | "tools"
  | "mcp"
  | "plugins"
  | "appearance"
  | "notices"
  | "about"
  | "models"
  | "rules"
  | "projectMcp"
  | "project"
  | "context";

type Item = { id: Section; name: Key; icon: ReactNode };

const GROUPS: { name: Key; items: Item[] }[] = [
  { name: "settings.general_group", items: [{ id: "appearance", name: "settings.general", icon: <Palette size={15} /> }] },
  { name: "settings.ai_group", items: [
    { id: "providers", name: "prov.title", icon: <Plugs size={15} /> },
    { id: "models", name: "settings.models", icon: <Cube size={15} /> },
  ] },
  { name: "settings.tools_group", items: [
    { id: "tools", name: "settings.environment", icon: <Wrench size={15} /> },
    { id: "mcp", name: "mcp.title", icon: <McpMark size={15} /> },
    { id: "plugins", name: "plug.title", icon: <PuzzlePiece size={15} /> },
  ] },
];
const PROJECT_SECTIONS: Item[] = [
  { id: "project", name: "settings.configuration", icon: <FolderSimple size={15} /> },
  { id: "context", name: "settings.context", icon: <BookOpen size={15} /> },
  { id: "rules", name: "settings.permissions", icon: <ShieldCheck size={15} /> },
];
const ABOUT: Item = { id: "about", name: "about.title", icon: <Info size={15} /> };

function isProjectSection(section: Section): boolean {
  return PROJECT_SECTIONS.some((item) => item.id === section);
}

export default function Settings({
  prefs: initial,
  section,
  onSection,
  onClose,
  onNewChat,
  narrow = false,
  startOnList = false,
  currentProvider,
  projects,
  currentProject,
  onProjectsChanged,
}: {
  prefs: Preferences;
  /** Held by the window, not here: the Help menu opens the layer on a section
      of its choosing, and moves an open one there too. */
  section: Section;
  onSection: (section: Section) => void;
  onClose: () => void;
  onNewChat?: (project: string) => void;
  /** One screen at a time: the list, or a section (`isNarrow`). */
  narrow?: boolean;
  /** Opened as «Settings» rather than on a section: a narrow window shows the list first. */
  startOnList?: boolean;
  currentProvider?: string;
  projects: ProjectEntry[];
  currentProject: string | null;
  onProjectsChanged: (projects: ProjectEntry[]) => void;
}) {
  const prefs = usePreferences(initial);
  // Narrow only: the list, a section picked from it, or a section the layer
  // was opened on.
  const [view, setView] = useState<"list" | "picked" | "given">(startOnList ? "list" : "given");
  useBackClose(narrow && view === "picked", () => setView("list"));
  const setSection = (next: Section) => {
    onSection(next);
    setView("picked");
  };
  const [project, setProject] = useState<string | null>(() => projects.find((entry) => entry.path === currentProject)?.path ?? projects[0]?.path ?? null);
  const [editingProject, setEditingProject] = useState(false);
  const entry = projects.find((entry) => entry.path === project) ?? null;
  const [models, setModels] = useState<ModelInfo[]>([]);

  useEffect(() => {
    setProject((now) => projects.some((entry) => entry.path === now) ? now : projects[0]?.path ?? null);
  }, [projects]);

  // Esc closes the layer while focus is not in a field: inside a field, Esc belongs to the field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (editingProject) return;
      const target = e.target as HTMLElement | null;
      if (target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName)) return;
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, editingProject]);

  const refreshModels = useCallback(() => {
    void fetchModels().then(setModels, () => setModels([]));
  }, []);

  useEffect(refreshModels, [refreshModels]);

  const patchPrefs = useCallback(
    (change: Partial<Preferences>) => {
      void setPreferences({ ...prefs, ...change }).catch((e: unknown) =>
        console.error("could not save the application settings", e),
      );
    },
    [prefs],
  );

  const projectSettings = useProjectSettings(project);
  const defaults = useModelDefaults(refreshModels);
  const update = useUpdate();
  const [mcpScope, setMcpScope] = useState<"global" | "project">("global");
  const ownMcp = useOwnMcp();
  const mcpFile = useMcpFile(project);

  // A project section has nothing to show without a project: the list is empty
  // until a folder has been opened.
  const normalized = section === "notices" ? "appearance" : section === "projectMcp" ? "mcp" : section;
  const shown = project === null && isProjectSection(normalized) ? "appearance" : normalized;
  useEffect(() => { if (section === "projectMcp") setMcpScope("project"); }, [section]);

  return (
    <><div
      className={
        !narrow
          ? "settings"
          : view === "list"
            ? "settings settings-list"
            : "settings settings-section"
      }
    >
      <nav className="set-nav" aria-label={t("set.nav")}>
        <button type="button" className="set-close" onClick={onClose}>
          <ArrowLeft size={15} aria-hidden="true" />
          <span>{t("set.close")}</span>
        </button>
        <div className="set-nav-scroll">
        {GROUPS.map((group) => <div className="set-nav-group" key={group.name}>
          <div className="set-group">{t(group.name)}</div>
          {group.items.map((item) => <NavItem key={item.id} item={item} active={shown} onPick={setSection} />)}
        </div>)}

        {project !== null && (
          <>
            <div className="set-group">{t("settings.project_group")}</div>
            {/* The switcher names the project whose repository settings are being edited. */}
            <Select
              align="start"
              className="set-project"
              label={t("set.project")}
              value={project}
              options={projects.map((entry) => ({
                value: entry.path,
                label: entry.name,
              }))}
              onChange={setProject}
            />
            {PROJECT_SECTIONS.map((item) => (
              <NavItem key={item.id} item={item} active={shown} onPick={setSection} />
            ))}
          </>
        )}
        </div>
        <div className="set-nav-footer">
          <NavItem item={ABOUT} active={shown} onPick={setSection} update={update.check?.latest != null} />
        </div>
      </nav>

      <main className="set-pane">
        {narrow && (
          <header className="set-up">
            <button
              type="button"
              className="top-icon"
              onClick={() => setView("list")}
              aria-label={t("set.title")}
            >
              <ArrowLeft size={18} aria-hidden="true" />
            </button>
            {t("set.title")}
          </header>
        )}
        <div className="set-scroll">
          {shown === "providers" && <ProvidersSection onChanged={refreshModels} currentProvider={currentProvider} />}
          {shown === "tools" && <ToolsSection prefs={prefs} onPatch={patchPrefs} />}
          {shown === "mcp" && <>
            <div className="set-scope-bar">
              <Segments label={t("settings.mcp_scope")} value={project ? mcpScope : "global"}
                options={[{ value: "global", label: t("settings.mcp_global") }, ...(project ? [{ value: "project", label: t("settings.mcp_project") }] : [])]}
                onChange={(scope) => setMcpScope(scope as "global" | "project")} />
              {project && <Select label={t("set.project")} value={project} options={projects.map((entry) => ({ value: entry.path, label: entry.name }))} onChange={setProject} />}
            </div>
            {mcpScope === "global" || !project ? <McpSection key="global" title={t("mcp.title")} lede={t("mcp.lede_app")}
              servers={ownMcp.servers} error={ownMcp.error} onSave={ownMcp.save} /> :
              <McpSection key={project} title={t("mcp.title")} lede={t("mcp.lede_project")}
                servers={projectSettings.settings?.mcpServers ?? null} error={projectSettings.error} inherited={mcpFile}
                onSave={(mcpServers) => projectSettings.patch((now) => ({ ...now, mcpServers }))} />}
          </>}
          {shown === "models" && <>
            {defaults.error && <div className="set-error" role="alert">{defaults.error}</div>}
            {defaults.values && <ModelsSection values={defaults.values} models={models} onPatch={defaults.patch} onChanged={refreshModels} />}
          </>}
          {shown === "plugins" && <PluginsSection onNewChat={project && onNewChat ? () => onNewChat(project) : undefined} />}
          {shown === "appearance" && <AppearanceSection prefs={prefs} onPatch={patchPrefs} />}
          {shown === "about" && <AboutSection prefs={prefs} onPatch={patchPrefs} />}
          {shown === "context" && project && <ContextSection key={project} project={project} />}

          {isProjectSection(shown) && project !== null && (
            <>
              {projectSettings.error !== null && (
                <div className="set-error">{projectSettings.error}</div>
              )}
              {projectSettings.settings !== null && (
                <>
                  {shown === "rules" && (
                    <><RulesSection
                      settings={projectSettings.settings}
                      onPatch={projectSettings.patch}
                    />
                    <details className="set-model-override"><summary>{t("settings.override")}</summary>
                      <p className="set-subhint">{t("settings.override_hint")}</p>
                      <ModelChoices inherited values={projectSettings.settings} models={models} onPatch={(change) => projectSettings.patch((now) => ({ ...now, ...change }))} />
                    </details></>
                  )}
                  {shown === "project" && (
                    <ProjectSection
                      project={project}
                      entry={entry}
                      onEdit={() => setEditingProject(true)}
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
    {editingProject && entry !== null && <ProjectEditor entry={entry} onClose={() => setEditingProject(false)} onSaved={(entries) => onProjectsChanged(entries)} />}
    </>
  );
}

/**
 * The human's own MCP servers: read once, written on every change.
 *
 * Shown as chosen even when the write fails, with the refusal beside it — the
 * same rule `useProjectSettings` follows, for the same reason.
 */
function useOwnMcp(): {
  servers: McpServers | null;
  error: string | null;
  save: (next: McpServers) => void;
} {
  const [servers, setServers] = useState<McpServers | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void globalMcpServers().then(setServers, (e: unknown) => setError(String(e)));
  }, []);

  const save = useCallback((next: McpServers) => {
    setServers(next);
    setError(null);
    void saveGlobalMcpServers(next).catch((e: unknown) => setError(String(e)));
  }, []);

  return { servers, error, save };
}

/** What the project's `.mcp.json` declares. A file that cannot be read shows
    as no file: the section only displays it, and its own list still works. */
function useMcpFile(project: string | null): McpServers {
  const [servers, setServers] = useState<McpServers>({});

  useEffect(() => {
    setServers({});
    if (project === null) return;
    let live = true;
    void projectMcpFile(project).then(
      (next) => live && setServers(next),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [project]);

  return servers;
}

function NavItem({
  item,
  active,
  onPick,
  update = false,
}: {
  item: Item;
  active: Section;
  onPick: (section: Section) => void;
  update?: boolean;
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
      {update && <span className="set-update-dot" role="img" aria-label={t("settings.update_available")} />}
    </button>
  );
}
