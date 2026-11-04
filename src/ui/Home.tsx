/**
 * Home screen §9.1: the window before a session exists.
 *
 * It owns its own state because nothing above it can: the greeting name and the
 * composer's settings are read or chosen before a session — and therefore
 * before `SessionState` — exists. The project is the exception. The sidebar
 * standing next to this screen lists that project's sessions, so the window
 * holds it and hands it down; two answers to «which project» would part ways
 * on the first folder picked. Starting the session goes up to the window too.
 *
 * The composer is the same component the session uses (§9.1), running on
 * settings this screen holds instead of ones the core reports. What is picked
 * here rides into the session with the first message.
 *
 * Provider editing (§10) lives in the panel next to the chips until the
 * settings window exists — an interim step recorded in §16.
 */

import { List } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import {
  chooseFiles,
  enabledPlugins,
  hasFileDialogs,
  projectStartup,
  rememberStartup,
  userName,
  type ModelInfo,
  type Startup,
} from "../core/client";
import { t } from "../i18n";
import type { MenuCommand } from "./slash";
import { Composer, type ComposerActions, type ComposerSettings } from "./Composer";
import { plainPath, relativeTo } from "./format";
import { imageActions } from "./images";
import { YakMascot } from "./YakMascot";
import type { Profile } from "../core/client";

export function Home({
  models,
  commands,
  onRefreshCommands,
  project,
  projectEntry,
  profile,
  error,
  onStart,
  onPickProject,
  onOpenSettings,
  onMenu,
}: {
  models: ModelInfo[];
  /** The project's slash commands §7.3 — the same composer, the same menu. */
  commands: MenuCommand[];
  onRefreshCommands: () => void;
  /** The project picked in the window: the session list beside it reads the same one (§9.1). */
  project: string | null;
  projectEntry: import("../core/client").ProjectEntry | null;
  profile: Profile;
  /** A session start error: it comes from the window, not from here. */
  error: string | null;
  onStart: (
    project: string,
    text: string,
    startup: Startup,
    attachments: string[],
  ) => void;
  /** The folder picker dialog belongs to the window — the project lives there. */
  onPickProject: () => void;
  /** The "Set up a provider" button leads to the settings window §10. */
  onOpenSettings: () => void;
  /** Opens the drawer the sidebar is in a narrow window; absent where it is a column. */
  onMenu?: () => void;
}) {
  const [name, setName] = useState<string | null>(null);
  const [startup, setStartup] = useState<Startup | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [keystrokes, setKeystrokes] = useState(0);

  useEffect(() => {
    void userName().then(setName);
  }, []);

  // Configured means the address answered with its models (§10). The first
  // provider to do so is where a project that names nothing starts, so the
  // defaults are asked again once there is one — after the provider panel,
  // not after a restart.
  const configured = models.length > 0;

  // What the project itself would start with (§10), under the last pick made
  // here, is the base the composer shows; the human overrides it below,
  // before the first word. With no
  // provider configured the shell refuses, and the composer stays shut with
  // the reason above it.
  useEffect(() => {
    if (project === null) return;
    let current = true;
    projectStartup(project).then(
      (defaults) => {
        if (current) {
          setStartup({ ...defaults, profile });
          setFailure(null);
        }
      },
      (e: unknown) => {
        if (current) setFailure(String(e));
      },
    );
    return () => {
      current = false;
    };
  }, [project, configured, profile]);

  const ready = project !== null && startup !== null;
  const settings: ComposerSettings = {
    mode: startup?.mode ?? "auto",
    profile,
    provider: startup?.provider ?? "",
    model: startup?.model ?? "",
    effort: startup?.effort ?? "extra",
    // Bypassing permissions is a decision about a running session, and there is nothing to bypass here.
    bypass: false,
  };

  // A pick is remembered for the next new chat, in this project or another.
  function change(patch: Partial<Startup>) {
    if (startup === null) return;
    const next = { ...startup, ...patch };
    setStartup(next);
    rememberStartup(next).catch((e: unknown) => console.error("could not remember the pick", e));
  }

  const actions: ComposerActions = {
    send: (text, attachments) => {
      if (project !== null && startup !== null) {
        onStart(project, text, { ...startup, profile }, attachments);
      }
    },
    // There is no stop button here: `busy` on the home screen is always false.
    stop: () => {},
    // A count rather than a flag: every keystroke must play the mascot's reaction again.
    typed: () => setKeystrokes((count) => count + 1),
    setMode: (mode) => change({ mode }),
    setProfile: (profile) => change({ profile }),
    setModel: (model, providerId) =>
      change(providerId ? { model, provider: providerId } : { model }),
    setEffort: (effort) => change({ effort }),
    // §7.2: the same `＋` as in a session. The root is known here too — the folder
    // is picked before the first message.
    pickFiles: hasFileDialogs
      ? async () => {
          if (project === null) return [];
          const picked = await chooseFiles(project, t("composer.attach"));
          return picked.map((path) => relativeTo(project, path));
        }
      : undefined,
    // Plugins do not need a session: what a new chat will load is already known.
    // MCP servers do, and there is no row for them on this screen.
    listPlugins: hasFileDialogs ? enabledPlugins : undefined,
    ...imageActions(project),
    // §7.3: `/compact` and `/clear` are about the conversation in progress, and
    // there is none here yet. Project commands work: the core expands them, and the
    // first message starts the session already expanded.
    refreshCommands: onRefreshCommands,
  };

  const shown = failure ?? error;

  return (
    <div className="home">
      <header className="home-top">
        {onMenu && (
          <button className="top-icon" onClick={onMenu} aria-label={t("side.toggle")}>
            <List size={20} />
          </button>
        )}
        {t("side.new")}
      </header>

      <div className="home-body">
        <div className="home-wrap">
          <div className="home-intro">
            <YakMascot keystrokes={keystrokes} />
            <div className="home-copy">
              <div className="home-greeting">
                <h1>{name === null ? t("home.greeting") : t("home.greeting_named", { name })}</h1>
              </div>
              <p className="home-lead">{t(profile === "chat" ? "home.lead_chat" : "home.lead")}</p>
            </div>
          </div>

          {profile === "code" && <div className="home-pick">
            <div className="home-pick-text">
              <div className="home-pick-title">{t("home.pick_title")}</div>
              <div className="home-project-name">
                {projectEntry?.name ?? (project === null ? t("home.not_chosen") : plainPath(project))}
              </div>
              {projectEntry && <div className="home-project-folders">{projectEntry.folders.map((folder) => <div className="home-pick-path" key={folder} title={plainPath(folder)}>{plainPath(folder)}</div>)}</div>}
            </div>
            <button className="btn" onClick={onPickProject} disabled={!hasFileDialogs}>
              {t("home.pick_project")}
            </button>
          </div>}

          {/* Before the first message a provider may be missing, and that is
              said here, not discovered by a refused send (§9.1, §10). */}
          {!configured && (
            <div className="home-note" role="status">
              <span className="home-note-text">
                <b>{t("home.no_provider")}</b> {t("home.no_provider_hint")}
              </span>
              <button className="btn btn-small" onClick={onOpenSettings} title={t("prov.title")}>
                {t("prov.home_button")}
              </button>
            </div>
          )}

          {shown !== null && <div className="home-error">{shown}</div>}

          <Composer
          settings={settings}
          busy={false}
          usage={null}
          models={models}
          commands={commands}
          queued={[]}
          found={null}
          actions={actions}
            disabled={!ready}
            draftKey={`yaca.draft.${project ?? "none"}`}
          />

        </div>
      </div>
    </div>
  );
}
