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

export function Home({
  models,
  commands,
  onRefreshCommands,
  project,
  error,
  onStart,
  onPickProject,
  onOpenSettings,
}: {
  models: ModelInfo[];
  /** Слэш-команды проекта §7.3 — тот же композер, то же меню. */
  commands: MenuCommand[];
  onRefreshCommands: () => void;
  /** Проект, выбранный в окне: список сессий рядом читает его же (§9.1). */
  project: string | null;
  /** Ошибка старта сессии: она приходит от окна, а не отсюда. */
  error: string | null;
  onStart: (
    project: string,
    text: string,
    startup: Startup,
    attachments: string[],
  ) => void;
  /** Диалог выбора папки принадлежит окну — проект живёт там. */
  onPickProject: () => void;
  /** Кнопка «Настроить провайдера» ведёт в окно настроек §10. */
  onOpenSettings: () => void;
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
    // Обход разрешений — решение о работающей сессии, а обходить здесь нечего.
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
    // Кнопки остановки здесь нет: `busy` на домашнем экране всегда ложь.
    stop: () => {},
    setMode: (mode) => change({ mode }),
    setModel: (model, providerId) =>
      change(providerId ? { model, provider: providerId } : { model }),
    setEffort: (effort) => change({ effort }),
    // §7.2: тот же `＋`, что и в сессии. Корень известен и здесь — папку
    // выбирают до первого сообщения.
    pickFiles: hasFileDialogs
      ? async () => {
          if (project === null) return [];
          const picked = await chooseFiles(project, t("composer.attach"));
          return picked.map((path) => relativeTo(project, path));
        }
      : undefined,
    ...imageActions(project),
    // §7.3: `/compact` и `/clear` — про идущий разговор, а его здесь ещё нет.
    // Проектные команды работают: их разворачивает ядро, и первое сообщение
    // заводит сессию уже развёрнутым.
    refreshCommands: onRefreshCommands,
  };

  const shown = failure ?? error;

  return (
    <div className="home">
      <header className="home-top">{t("side.new")}</header>

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
