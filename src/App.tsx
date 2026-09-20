/**
 * Окно: каркас §1 и всё, что в него вставлено.
 *
 * Здесь нет ни одного собственного «состояния ленты»: всё, что рисуется,
 * выводится из потока событий редьюсером `core/feed`. Лент теперь несколько —
 * по одной на открытый чат (`ui/chats`), — и живут они все сразу: ход,
 * начатый в одном разговоре, идёт дальше, пока смотрят в другой.
 *
 * Своё состояние тут только то, что живёт в окне и ядру неизвестно: список
 * проектов, проект домашнего экрана, показан ли сайдбар, какая панель §1.1
 * открыта, какой файл в ней показан.
 *
 * Панели §1.1 — одна строка состояния, а не три независимых Dialog: открыта
 * всегда одна, и закрытая панель размонтируется, не помня себя (§1.1).
 * Сайдбар в этот счёт не входит: он не выдвижная панель, а левая колонка
 * каркаса, и стоит рядом с лентой, а не поверх неё.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  addProject as rememberProject,
  chooseFiles,
  chooseFolder,
  commands,
  forgetProject as dropProject,
  hasFileDialogs,
  models as fetchModels,
  projectCommands,
  projects as listProjects,
  sessions,
  type Command,
  type Effort,
  type ModelInfo,
  type PermissionAnswer,
  type ProcessId,
  type SlashCommand,
} from "./core/client";
import { touchedFiles, visible } from "./core/feed";
import { relativeTo } from "./ui/format";
import { t } from "./i18n";
import { Feed, type FeedActions, type FeedJump } from "./ui/Feed";
import { Notices } from "./ui/Bars";
import { Composer, MODES, type ComposerActions } from "./ui/Composer";
import { Home } from "./ui/Home";
import { TopBar } from "./ui/TopBar";
import { Sidebar } from "./ui/sidebar/Sidebar";
import { ProcessesPanel } from "./ui/panel/ProcessesPanel";
import { FilePanel } from "./ui/panel/FilePanel";
import { CheckpointsPanel } from "./ui/panel/CheckpointsPanel";
import { ProvidersPanel } from "./ui/panel/ProvidersPanel";
import { useChats } from "./ui/chats";
import { useShortcuts } from "./ui/keyboard";

import "@fontsource/unbounded/600.css";
import "@fontsource/onest/400.css";
import "@fontsource/onest/500.css";
import "@fontsource/onest/600.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/600.css";
import "./ui/tokens.css";
import "./ui/app.css";
import "./ui/feed.css";
import "./ui/diff.css";
import "./ui/syntax.css";
import "./ui/home.css";
import "./ui/panel/panel.css";
import "./ui/sidebar/sidebar.css";

/** Панель §1.1. Сессии уехали в свою колонку — сайдбар, а не панель. */
type PanelKind = "checkpoints" | "file" | "processes" | "providers";

export default function App() {
  const [error, setError] = useState<string | null>(null);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [projects, setProjects] = useState<string[]>([]);
  const [slashCommands, setSlashCommands] = useState<SlashCommand[]>([]);
  const [draft, setDraft] = useState<string | null>(null);
  const [panel, setPanel] = useState<PanelKind | null>(null);
  const [filePanelPath, setFilePanelPath] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);

  const failed = useCallback((e: unknown) => setError(String(e)), []);
  const chats = useChats(failed);
  const active = chats.active;

  // Ручной скачок по ленте идёт через её окно: блок далеко от видимой области
  // не размечен, и `getElementById` на нём мёртв. Скачки тут (стрелки шапки §3) —
  // это и возврат по чекпоинтам, поэтому ручка одна и та же для обоих путей.
  const feedJump = useRef<FeedJump>(null);

  /**
   * Слэш-команды §7.3 того проекта, в котором сейчас пишут.
   *
   * Список нужен только меню: разворачивает команду ядро, читая каталог на
   * каждом сообщении. Поэтому и перечитывается он по требованию композера, а
   * не подпиской: файлы проекта живут вне потока `AgentEvent`.
   */
  const project = active?.project ?? draft;
  const refreshCommands = useCallback(() => {
    if (project === null) return setSlashCommands([]);
    void projectCommands(project).then(setSlashCommands, failed);
  }, [project, failed]);

  useEffect(refreshCommands, [refreshCommands]);

  useEffect(() => {
    // Проекты помнят глобальные настройки приложения: в самом проекте список
    // хранить негде, он нужен раньше, чем известен хоть один проект.
    void listProjects().then((list) => {
      setProjects(list);
      setDraft((now) => now ?? list[0] ?? null);
    }, failed);
  }, [failed]);

  /** Модели §7.1 меняются в панели провайдеров и перечитываются после неё. */
  const refreshProviders = useCallback(() => {
    // Ошибку глотать нельзя: битый `providers.json` останавливает список
    // моделей, и молча пустой переключатель §7.1 не объяснил бы, почему он
    // пуст.
    void fetchModels().then(setModels, failed);
  }, [failed]);

  useEffect(refreshProviders, [refreshProviders]);

  const closeProviders = useCallback(() => {
    setPanel(null);
    refreshProviders();
  }, [refreshProviders]);

  const togglePanel = useCallback(
    (kind: PanelKind) => setPanel((now) => (now === kind ? null : kind)),
    [],
  );

  // --- проекты и чаты -------------------------------------------------------

  /**
   * «Новый чат»: окно показывает домашний экран §9.1 этого проекта.
   *
   * Сессия рождается первым сообщением, и кнопка, заводящая её сразу,
   * оставляла бы в списке пустую строку за каждое «передумал» (§9.1).
   * Начатые чаты при этом никуда не деваются: они в сайдбаре рядом.
   */
  const newChat = useCallback(
    (project: string) => {
      setDraft(project);
      setPanel(null);
      chats.activate(null);
    },
    [chats],
  );

  const addProject = useCallback(async () => {
    const picked = await chooseFolder(t("home.pick_title"));
    if (picked === null) return;
    try {
      const list = await rememberProject(picked);
      setProjects(list);
      // Добавили — значит сюда и собирались: домашний экран открывается в нём.
      setDraft(list[0] ?? picked);
      chats.activate(null);
    } catch (e) {
      failed(e);
    }
  }, [chats, failed]);

  const forgetProject = useCallback(
    async (project: string) => {
      try {
        const list = await dropProject(project);
        setProjects(list);
        setDraft((now) => (now === project ? (list[0] ?? null) : now));
      } catch (e) {
        failed(e);
      }
    },
    [failed],
  );

  // --- действия ленты и композера -------------------------------------------

  const humanKeys = useMemo(
    () =>
      active ? visible(active.feed).filter((b) => b.kind === "human").map((b) => b.key) : [],
    [active],
  );

  const send = useCallback(
    (command: Command) => {
      if (active) chats.send(active.id, command);
    },
    [active, chats],
  );

  const jumpTo = useCallback(
    (index: number) => {
      const key = humanKeys[index];
      if (!active || !key) return;
      chats.patch(active.id, { at: index });
      feedJump.current?.jumpTo(key);
    },
    [active, chats, humanKeys],
  );

  const lastCheckpoint = useCallback(
    (kind: "message") => {
      if (!active) return null;
      const points = active.feed.checkpoints;
      for (let i = points.length - 1; i >= 0; i -= 1) {
        if (points[i].reason.type === kind) return points[i].id;
      }
      return null;
    },
    [active],
  );

  const feedActions: FeedActions = {
    answer: (answer: PermissionAnswer) => {
      if (active?.feed.request) {
        send(commands.answerPermission(active.feed.request.id, answer));
      }
    },
    previewCheckpoint: (id) => send(commands.previewCheckpoint(id)),
    openCheckpoints: () => setPanel("checkpoints"),
    restoreCheckpoint: (id) => send(commands.restoreCheckpoint(id)),
    cancelPreview: () => {
      if (active) chats.patch(active.id, { feed: { ...active.feed, preview: null } });
    },
    editSummary: (text) => send(commands.editSummary(text)),
    // Панель файла открывается и по имени файла в строке действия (§1.1).
    openFile: (path) => {
      setFilePanelPath(path);
      setPanel("file");
    },
    retry: () => {
      if (!active) return;
      const last = [...visible(active.feed)].reverse().find((b) => b.kind === "human");
      if (last && last.kind === "human") send(commands.send(last.text));
    },
  };

  const composerActions: ComposerActions = {
    send: (text, attachments) => {
      if (!active) return;
      if (active.feed.state?.busy) chats.patch(active.id, { queued: text });
      send(commands.send(text, attachments));
    },
    stop: () => send(commands.cancel()),
    setMode: (mode) => send(commands.setMode(mode)),
    setBypass: (on) => send(commands.setBypass(on)),
    setModel: (model) => send(commands.setModel(model)),
    setEffort: (effort: Effort) => send(commands.setEffort(effort)),
    // §7.3: встроенные команды делает интерфейс — это те же действия, что у
    // полосы §6.1.1 и у строки «Новый чат» сайдбара.
    compact: () => send(commands.compact()),
    openCheckpoints: () => setPanel("checkpoints"),
    newChat: () => {
      if (active !== null) newChat(active.project);
    },
    // §7.2: пути отдаются от корня проекта — их же увидит модель, и карточка
    // §5.1 не растянется абсолютным путём.
    pickFiles: hasFileDialogs
      ? async () => {
          const root = active?.feed.state?.projectRoot;
          if (root === undefined) return [];
          const picked = await chooseFiles(root, t("composer.attach"));
          return picked.map((path) => relativeTo(root, path));
        }
      : undefined,
    refreshCommands,
  };

  // --- панели §1.1 ----------------------------------------------------------

  const watchProcess = useCallback(
    (id: ProcessId | null) => {
      if (!active) return;
      chats.patch(active.id, { watched: id });
      chats.send(active.id, commands.watchProcess(id));
    },
    [active, chats],
  );

  // ⌥← / ⌥→ листают файлы текущего набора, не закрывая панель (§6.4).
  const shiftFile = useCallback(
    (delta: number) => {
      if (panel !== "file" || !active) return;
      const paths = touchedFiles(active.feed);
      if (paths.length === 0) return;
      const at = paths.indexOf(filePanelPath ?? "");
      const next = Math.min(paths.length - 1, Math.max(0, at === -1 ? 0 : at + delta));
      setFilePanelPath(paths[next]);
    },
    [panel, filePanelPath, active],
  );

  useShortcuts({
    suspended: active?.feed.request != null,
    panelOpen: panel !== null,
    closePanel: () => setPanel(null),
    prevFile: () => shiftFile(-1),
    nextFile: () => shiftFile(1),
    stop: () => {
      if (active?.feed.state?.busy) send(commands.cancel());
    },
    rollbackToPreviousMessage: () => {
      const id = lastCheckpoint("message");
      if (id) send(commands.previewCheckpoint(id));
    },
    nextMode: () => {
      // На домашнем экране сочетание молчит: сессии нет, `setMode` слать
      // некуда, а чип там свой и живёт в состоянии `Home` (§9.1). Клавиатуру
      // до него не дотянули — она вся висит здесь, — и это видимая дыра, а не
      // забытая ветка: мышью режим до старта выбирается.
      const now = active?.feed.state?.mode;
      if (now === undefined) return;
      composerActions.setMode(MODES[(MODES.indexOf(now) + 1) % MODES.length]);
    },
  });

  // --- разметка -------------------------------------------------------------

  // Сайдбар один и тот же на обоих экранах: разное — то, что стоит справа.
  const sidebar = (
    <Sidebar
      projects={projects}
      chats={chats.chats}
      activeId={active?.id ?? null}
      current={active?.project ?? draft}
      // Перечитать списки есть смысл, когда они могли измениться: открыли или
      // закрыли чат, закончился ход и сменился заголовок.
      reload={chats.chats.map((chat) => `${chat.id}:${chat.title}`).join("|")}
      providersOpen={panel === "providers"}
      onNewChat={newChat}
      onOpenSession={(project, id) => void chats.resume(project, id).catch(failed)}
      onCloseChat={(id) => void chats.close(id).catch(failed)}
      onAddProject={() => void addProject()}
      onForgetProject={(project) => void forgetProject(project)}
      onOpenProviders={() => togglePanel("providers")}
    />
  );

  // Состояние приезжает вместе с открытым чатом, поэтому `null` здесь значит
  // ровно одно: открытого чата нет и окно на домашнем экране §9.1.
  const state = active?.feed.state ?? null;

  if (active === null || state === null) {
    return (
      <div className="shell">
        {/* На домашнем экране сайдбар не прячется: прячет его кнопка шапки
            §3, а шапки здесь нет. Прятать тут и нечего — кроме сайдбара на
            домашнем экране никакой навигации не осталось. */}
        {sidebar}

        <Home
          models={models}
          commands={slashCommands}
          onRefreshCommands={refreshCommands}
          project={draft}
          error={error}
          onStart={(path, text, startup, attachments) =>
            void chats.start(path, text, startup, attachments).catch(failed)
          }
          onPickProject={() => void addProject()}
          onOpenProviders={() => togglePanel("providers")}
        />

        {/* На домашнем экране верхней панели нет — панель на всю высоту. */}
        {panel === "providers" && (
          <ProvidersPanel withoutTopBar onClose={closeProviders} />
        )}
      </div>
    );
  }

  const runningProcesses = active.feed.processes.filter(
    (p) => p.state.type === "running",
  ).length;

  return (
    <div className="shell">
      {sidebarOpen && sidebar}

      <div className="app">
        <TopBar
          title={active.title}
          projectRoot={state.projectRoot}
          canGoBack={humanKeys.length > 0 && active.at !== 0}
          canGoForward={active.at >= 0 && active.at < humanKeys.length - 1}
          onBack={() => jumpTo(active.at <= 0 ? humanKeys.length - 1 : active.at - 1)}
          onForward={() => jumpTo(active.at + 1)}
          onRename={(next) => {
            void sessions
              .rename(active.project, active.id, next)
              .then(() => chats.patch(active.id, { title: next }), failed);
          }}
          sidebarOpen={sidebarOpen}
          panel={panel === "processes" ? panel : null}
          runningProcesses={runningProcesses}
          onToggleSidebar={() => setSidebarOpen((now) => !now)}
          onToggleProcesses={() => togglePanel("processes")}
        />

        {panel === "processes" && (
          <ProcessesPanel
            feed={active.feed}
            watched={active.watched}
            onClose={() => setPanel(null)}
            onWatch={watchProcess}
            onStop={(id) => send(commands.stopProcess(id))}
          />
        )}

        {panel === "file" && filePanelPath !== null && (
          <FilePanel path={filePanelPath} feed={active.feed} onClose={() => setPanel(null)} />
        )}

        {panel === "checkpoints" && (
          <CheckpointsPanel
            feed={active.feed}
            onClose={() => setPanel(null)}
            onPreview={feedActions.previewCheckpoint}
            onRestore={feedActions.restoreCheckpoint}
            onCancel={feedActions.cancelPreview}
          />
        )}

        {panel === "providers" && (
          <ProvidersPanel onClose={closeProviders} />
        )}

        <Feed feed={active.feed} actions={feedActions} ref={feedJump} />

        <div className="bars">
          <Notices feed={active.feed} onCompact={() => send(commands.compact())} />
        </div>

        <Composer
          settings={{
            mode: state.mode,
            provider: state.provider,
            model: state.model,
            effort: state.effort,
            bypass: state.bypass,
          }}
          busy={state.busy}
          ring={active.feed.request ? "waiting" : state.busy ? "busy" : "idle"}
          usage={active.feed.usage}
          models={
            state.provider
              ? models.filter((m) => !m.providerId || m.providerId === state.provider)
              : models
          }
          commands={slashCommands}
          queued={active.queued}
          actions={composerActions}
          // Черновик принадлежит разговору, а не проекту: чатов в проекте
          // несколько, и недописанное в одном не должно всплывать в другом.
          draftKey={`yaca.draft.${active.id}`}
        />
      </div>
    </div>
  );
}
