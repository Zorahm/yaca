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

import { Gear } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import {
  chooseFiles,
  hasFileDialogs,
  projectStartup,
  userName,
  type ModelInfo,
  type SlashCommand,
  type Startup,
} from "../core/client";
import { t } from "../i18n";
import { Composer, type ComposerActions, type ComposerSettings } from "./Composer";
import { relativeTo } from "./format";
import { ProjectChip } from "./ContextChips";
import { Mark } from "./Mark";

export function Home({
  models,
  commands,
  onRefreshCommands,
  project,
  error,
  onStart,
  onPickProject,
  onOpenProviders,
}: {
  models: ModelInfo[];
  /** Слэш-команды проекта §7.3 — тот же композер, то же меню. */
  commands: SlashCommand[];
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
  /** Кнопка «Настроить провайдера»: панель §1.1 держит App, не этот экран. */
  onOpenProviders: () => void;
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
    mode: startup?.mode ?? "manual",
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
    // §7.3: `/compact` и `/clear` — про идущий разговор, а его здесь ещё нет.
    // Проектные команды работают: их разворачивает ядро, и первое сообщение
    // заводит сессию уже развёрнутым.
    refreshCommands: onRefreshCommands,
  };

  const shown = failure ?? error;

  return (
    <div className="home">
      <div className="home-greeting">
        <Mark />
        <h1>{name === null ? t("home.greeting") : t("home.greeting_named", { name })}</h1>
      </div>

      <div className="home-bottom">
        {shown !== null && <div className="home-error">{shown}</div>}

        <div className="home-chips-wrap">
          <div className="home-chips">
            <ProjectChip
              path={project}
              onPick={onPickProject}
              disabled={!hasFileDialogs}
            />
            {/* Триггер панели §1.1: атрибут нужен, чтобы клик по открытой
                панели закрыл её, а не открыл заново (§1.1). */}
            <button
              className="chip-context"
              onClick={onOpenProviders}
              data-panel-trigger={true}
              title={t("prov.title")}
            >
              <span className="chip-icon" aria-hidden="true">
                <Gear size={13} />
              </span>
              {t("prov.home_button")}
            </button>
          </div>
        </div>

        <Composer
          settings={settings}
          busy={false}
          ring="idle"
          usage={null}
          models={models}
          commands={commands}
          queued={null}
          actions={actions}
          disabled={!ready}
          draftKey={`yaca.draft.${project ?? "none"}`}
        />
      </div>
    </div>
  );
}
