/**
 * Settings §10, over the main window.
 *
 * Not a separate window: it was created by a shell command, and on Windows it
 * hung the whole program and also hid the conversation. Now it is a layer over
 * the window, and it closes with the button in the navigation or with Esc.
 *
 * The navigation is split in two: the application and the project. §10 lists
 * the sections in one list, but that list mixes two levels of truth: providers,
 * appearance and notices are common to everything, while models, mode, rules and
 * snapshot exclusions belong to this project. With a flat list a person would
 * change "Mode" without knowing which one it was, so the project group has a
 * folder chip, and it is also the switcher.
 *
 * There are eight sections, not nine: there are no language servers, because
 * nothing in the core stands behind them, and a section that changes nothing is
 * an interface lying about the state (§0). MCP is in both groups — its own
 * servers for the application, its own for the project — and it is one and the
 * same section over two lists.
 *
 * Settings apply at once, without a "Save" button: it would let the window show
 * one thing while the disk held another. The exception is the provider's
 * description: it is a document, and an unfinished address must not reach the
 * disk.
 */

import {
  ArrowLeft,
  Bell,
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
  projects as listProjects,
  saveGlobalMcpServers,
  setPreferences,
  type ModelInfo,
  type Preferences,
} from "./core/client";
import { t, type Key } from "./i18n";
import { McpMark } from "./ui/brand";
import { plainPath } from "./ui/format";
import { usePreferences } from "./ui/prefs";
import { AboutSection } from "./ui/settings/AboutSection";
import { AppearanceSection } from "./ui/settings/AppearanceSection";
import { McpSection, type McpServers } from "./ui/settings/McpSection";
import { ModelsSection } from "./ui/settings/ModelsSection";
import { NoticesSection } from "./ui/settings/NoticesSection";
import { PluginsSection } from "./ui/settings/PluginsSection";
import { ProjectSection } from "./ui/settings/ProjectSection";
import { ProvidersSection } from "./ui/settings/ProvidersSection";
import { Select } from "./ui/settings/Row";
import { RulesSection } from "./ui/settings/RulesSection";
import { ToolsSection } from "./ui/settings/ToolsSection";
import { useProjectSettings } from "./ui/settings/useProjectSettings";

import "./ui/fonts";
import "./ui/tokens.css";
import "./ui/app.css";
import "./ui/popmenu.css";
import "./ui/settings/settings.css";

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
  | "project";

type Item = { id: Section; name: Key; icon: ReactNode };

const APP_SECTIONS: Item[] = [
  { id: "providers", name: "prov.title", icon: <Plugs size={15} /> },
  { id: "tools", name: "tools.title", icon: <Wrench size={15} /> },
  { id: "mcp", name: "mcp.title", icon: <McpMark size={15} /> },
  { id: "plugins", name: "plug.title", icon: <PuzzlePiece size={15} /> },
  { id: "appearance", name: "look.title", icon: <Palette size={15} /> },
  { id: "notices", name: "notices.title", icon: <Bell size={15} /> },
  { id: "about", name: "about.title", icon: <Info size={15} /> },
];

const PROJECT_SECTIONS: Item[] = [
  { id: "models", name: "models.title", icon: <Cube size={15} /> },
  { id: "rules", name: "rules.title", icon: <ShieldCheck size={15} /> },
  { id: "projectMcp", name: "mcp.title", icon: <McpMark size={15} /> },
  { id: "project", name: "proj.title", icon: <FolderSimple size={15} /> },
];

/** U+200E, the left-to-right mark. By code: the character itself is invisible in source. */
const LTR_MARK = String.fromCharCode(0x200e);

function isProjectSection(section: Section): boolean {
  return PROJECT_SECTIONS.some((item) => item.id === section);
}

export default function Settings({
  prefs: initial,
  section,
  onSection,
  onClose,
}: {
  prefs: Preferences;
  /** Held by the window, not here: the Help menu opens the layer on a section
      of its choosing, and moves an open one there too. */
  section: Section;
  onSection: (section: Section) => void;
  onClose: () => void;
}) {
  const prefs = usePreferences(initial);
  const setSection = onSection;
  const [projects, setProjects] = useState<string[]>([]);
  const [project, setProject] = useState<string | null>(null);
  const [models, setModels] = useState<ModelInfo[]>([]);

  useEffect(() => {
    // Newest first — the same order in which the main window picks the default
    // project, so the settings window opens on that same one.
    void listProjects().then((list) => {
      setProjects(list);
      setProject((now) => now ?? list[0] ?? null);
    });
  }, []);

  // Esc closes the layer while focus is not in a field: inside a field, Esc belongs to the field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      const target = e.target as HTMLElement | null;
      if (target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName)) return;
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

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
  const ownMcp = useOwnMcp();
  const mcpFile = useMcpFile(project);

  // A project section has nothing to show without a project: the list is empty
  // until a folder has been opened.
  const shown = project === null && isProjectSection(section) ? "providers" : section;

  return (
    <div className="settings">
      <nav className="set-nav" aria-label={t("set.nav")}>
        <button type="button" className="set-close" onClick={onClose}>
          <ArrowLeft size={15} aria-hidden="true" />
          <span>{t("set.close")}</span>
        </button>
        <div className="set-group">{t("set.group_app")}</div>
        {APP_SECTIONS.map((item) => (
          <NavItem key={item.id} item={item} active={shown} onPick={setSection} />
        ))}

        {project !== null && (
          <>
            <div className="set-group">{t("set.group_project")}</div>
            {/* The project chip is also the switcher: there are several folders in
                the list, and the section must say which one it edits. */}
            <Select
              mono
              align="start"
              className="set-project"
              label={t("set.project")}
              value={project}
              // Shown without the `\\?\` prefix, like everywhere else a path is
              // read. The mark in front pins the text left-to-right: the chip
              // is `rtl` to cut a long path on the left, and without it the
              // slashes a path starts with are drawn at its far end.
              options={projects.map((path) => ({
                value: path,
                label: LTR_MARK + plainPath(path),
              }))}
              onChange={setProject}
            />
            {PROJECT_SECTIONS.map((item) => (
              <NavItem key={item.id} item={item} active={shown} onPick={setSection} />
            ))}
          </>
        )}
      </nav>

      <main className="set-pane">
        <div className="set-scroll">
          {shown === "providers" && <ProvidersSection models={models} onChanged={refreshModels} />}
          {shown === "tools" && <ToolsSection prefs={prefs} onPatch={patchPrefs} />}
          {shown === "mcp" && (
            <McpSection
              title={t("mcp.title")}
              lede={t("mcp.lede_app")}
              servers={ownMcp.servers}
              error={ownMcp.error}
              onSave={ownMcp.save}
            />
          )}
          {shown === "plugins" && <PluginsSection />}
          {shown === "appearance" && <AppearanceSection prefs={prefs} onPatch={patchPrefs} />}
          {shown === "notices" && <NoticesSection prefs={prefs} onPatch={patchPrefs} />}
          {shown === "about" && <AboutSection prefs={prefs} onPatch={patchPrefs} />}

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
                  {shown === "projectMcp" && (
                    <McpSection
                      title={t("mcp.title")}
                      lede={t("mcp.lede_project")}
                      servers={projectSettings.settings.mcpServers}
                      error={null}
                      inherited={mcpFile}
                      onSave={(mcpServers) =>
                        projectSettings.patch((now) => ({ ...now, mcpServers }))
                      }
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
