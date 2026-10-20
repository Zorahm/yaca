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

import { useEffect, useState } from "react";

import {
  chooseFiles,
  hasFileDialogs,
  projectStartup,
  userName,
  type ModelInfo,
  type Startup,
} from "../core/client";
import { t } from "../i18n";
import type { MenuCommand } from "./slash";
import { Composer, type ComposerActions, type ComposerSettings } from "./Composer";
import { plainPath, relativeTo } from "./format";
import { imageActions } from "./images";
import { PanelLeft } from "./yaca-icons";

export function Home({
  models,
  commands,
  onRefreshCommands,
  project,
  error,
  onStart,
  onPickProject,
  onOpenSettings,
  onOpenSidebar,
}: {
  models: ModelInfo[];
  /** The project's slash commands §7.3 — the same composer, the same menu. */
  commands: MenuCommand[];
  onRefreshCommands: () => void;
  /** The project picked in the window: the session list beside it reads the same one (§9.1). */
  project: string | null;
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
  /** Where the sidebar is a drawer there is no column to see it in: this opens it. */
  onOpenSidebar?: () => void;
}) {
  const [name, setName] = useState<string | null>(null);
  const [startup, setStartup] = useState<Startup | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    void userName().then(setName);
  }, []);

  // Configured means the address answered with its models (§10). The first
  // provider to do so is where a project that names nothing starts, so the
  // defaults are asked again once there is one — after the provider panel,
  // not after a restart.
  const configured = models.length > 0;

  // What the project itself would start with (§10) is the base the composer
  // shows; the human overrides it below, before the first word. With no
  // provider configured the shell refuses, and the composer stays shut with
  // the reason above it.
  useEffect(() => {
    if (project === null) return;
    let current = true;
    projectStartup(project).then(
      (defaults) => {
        if (current) {
          setStartup(defaults);
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
  }, [project, configured]);

  const ready = project !== null && startup !== null;
  const settings: ComposerSettings = {
    mode: startup?.mode ?? "auto",
    provider: startup?.provider ?? "",
    model: startup?.model ?? "",
    effort: startup?.effort ?? "extra",
    // Bypassing permissions is a decision about a running session, and there is nothing to bypass here.
    bypass: false,
  };

  function change(patch: Partial<Startup>) {
    setStartup((now) => (now === null ? now : { ...now, ...patch }));
  }

  const actions: ComposerActions = {
    send: (text, attachments) => {
      if (project !== null && startup !== null) {
        onStart(project, text, startup, attachments);
      }
    },
    // There is no stop button here: `busy` on the home screen is always false.
    stop: () => {},
    setMode: (mode) => change({ mode }),
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
        {onOpenSidebar !== undefined && (
          <button
            className="top-icon"
            onClick={onOpenSidebar}
            aria-label={t("side.toggle")}
            title={t("side.toggle")}
          >
            <PanelLeft size={18} open={false} />
          </button>
        )}
        {t("side.new")}
      </header>

      <div className="home-body">
        <div className="home-wrap">
          <div className="home-greeting">
            <h1>{name === null ? t("home.greeting") : t("home.greeting_named", { name })}</h1>
          </div>
          <p className="home-lead">{t("home.lead")}</p>

          {/* The project is the folder: a card that names it and the one
              button that changes it (§9). */}
          <div className="home-pick">
            <div className="home-pick-text">
              <div className="home-pick-title">{t("home.pick_title")}</div>
              <div className="home-pick-path" title={project ?? undefined}>
                {project === null ? t("home.not_chosen") : plainPath(project)}
              </div>
            </div>
            <button className="btn" onClick={onPickProject} disabled={!hasFileDialogs}>
              {t("home.pick_project")}
            </button>
          </div>

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
