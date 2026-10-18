/**
 * Окно: каркас и всё, что в него вставлено.
 *
 * Здесь нет ни одного собственного «состояния ленты»: всё, что рисуется,
 * выводится из потока событий редьюсером `core/feed`. Лент теперь несколько —
 * по одной на открытый чат (`ui/chats`), — и живут они все сразу: ход,
 * начатый в одном разговоре, идёт дальше, пока смотрят в другой.
 *
 * Своё состояние тут только то, что живёт в окне и ядру неизвестно: список
 * проектов, проект домашнего экрана, показан ли сайдбар, какая панель
 * открыта, какой файл в ней показан.
 *
 * Правая панель — одна строка состояния, а не четыре независимых флага:
 * открыта всегда одна, и закрытая панель размонтируется, не помня себя.
 * Она стоит колонкой справа от ленты, как сайдбар слева; ширину обеих
 * человек тянет сам, и помнит её окно (`ui/resize`).
 *
 * Between the feed and the panel stands the browser column, with its own
 * button and its own width (`ui/browser`).
 */

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import {
  addProject as rememberProject,
  chooseFiles,
  chooseFolder,
  commands,
  forgetProject as dropProject,
  hasFileDialogs,
  models as fetchModels,
  onProviders,
  projectCommands,
  projectSkills,
  projects as listProjects,
  sessions,
  type Command,
  type Effort,
  type ModelInfo,
  type PermissionAnswer,
  type QuestionReply,
  type Preferences,
  type ProcessId,
} from "./core/client";
import { agentsOf } from "./core/agents";
import { touchedFiles, visible } from "./core/feed";
import { relativeTo } from "./ui/format";
import { t } from "./i18n";
import { Feed, type FeedActions, type FeedJump } from "./ui/Feed";
import { Notices } from "./ui/Bars";
import { Composer, type ComposerActions } from "./ui/Composer";
import { MODES } from "./ui/ModeMenu";
import { mcpPrompts, withSkills, type MenuCommand } from "./ui/slash";
import { Home } from "./ui/Home";
import { imageActions } from "./ui/images";
import { galleryOf } from "./ui/viewer/gallery";
import { Pictures } from "./ui/viewer/Viewer";
import { TitleBar } from "./ui/TitleBar";
import { TopBar } from "./ui/TopBar";
import { Sidebar } from "./ui/sidebar/Sidebar";
import { ProcessesPanel } from "./ui/panel/ProcessesPanel";
import { McpPanel } from "./ui/panel/McpPanel";
import { LspPanel } from "./ui/panel/LspPanel";
import Settings, { type Section as SettingsSection } from "./Settings";
import { ownTitleBar } from "./core/frame";
import { setHelpMenu } from "./core/menu";
import { usePreferences } from "./ui/prefs";
import { FilePanel } from "./ui/panel/FilePanel";
import { PanelFrameContext, type PanelKind } from "./ui/panel/Panel";
import { CheckpointsPanel } from "./ui/panel/CheckpointsPanel";
import { AgentsPanel } from "./ui/panel/AgentsPanel";
import { useChats } from "./ui/chats";
import { BrowserColumn } from "./ui/browser/BrowserColumn";
import { useBrowserCover } from "./ui/browser/cover";
import { useBrowserColumn } from "./ui/browser/useBrowserColumn";
import { useShortcuts } from "./ui/keyboard";
import {
  clamp,
  FILE_SHARE,
  PANEL,
  panelBounds,
  SIDE,
  sideBounds,
  useStoredWidth,
  useWindowWidth,
} from "./ui/resize";

import "./ui/fonts";
import "./ui/tokens.css";
import "./ui/app.css";
import "./ui/splitter.css";
import "./ui/popmenu.css";
import "./ui/agents.css";
import "./ui/feed.css";
import "./ui/diff.css";
import "./ui/syntax.css";
import "./ui/home.css";
import "./ui/panel/panel.css";
import "./ui/viewer/viewer.css";
import "./ui/sidebar/sidebar.css";

export default function App({ prefs }: { prefs: Preferences }) {
  // Настройки приложения живут в оболочке: подписка ставит атрибуты на
  // <html> и перерисовывает дерево, когда выбрали другую тему или язык.
  const livePrefs = usePreferences(prefs);
  // Настройки поверх окна, а не отдельным окном: второе окно создавалось из
  // команды оболочки и на Windows вешало всю программу, а ещё оно прятало
  // разговор, к которому человек вернётся через секунду.
  // Which section it stands on, or `null` while it is closed.
  const [settingsAt, setSettingsAt] = useState<SettingsSection | null>(null);

  // Both items of the Help menu lead to the same screen, on purpose: the row
  // there says that nothing is published to check against yet
  // (`AboutSection`).
  const openAbout = useCallback(() => setSettingsAt("about"), []);

  // Where the window keeps the system's frame, Help is the system's menu
  // (`core/menu`); where it draws its own, Help is in the title bar below.
  // Rebuilt when the language changes: its labels are the dictionary's, and
  // the menu is the one place a stale language would stay on screen with
  // nothing to redraw it.
  useEffect(() => {
    void setHelpMenu({
      title: t("menu.help"),
      items: [
        { id: "check-updates", text: t("menu.check_updates"), action: openAbout },
        { id: "about", text: t("menu.about"), action: openAbout },
      ],
    }).catch((e: unknown) => console.error("не собрать системное меню", e));
  }, [livePrefs.language, openAbout]);

  // The window: its own title bar, where it has one, over everything else —
  // the layers that cover the window included (`titlebar.css`).
  const frame = (body: ReactNode) => (
    <div className={ownTitleBar ? "window window-framed" : "window"}>
      {ownTitleBar && <TitleBar onCheckUpdates={openAbout} onAbout={openAbout} />}
      <div className="window-body">{body}</div>
    </div>
  );

  const [error, setError] = useState<string | null>(null);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [projects, setProjects] = useState<string[]>([]);
  const [slashCommands, setSlashCommands] = useState<MenuCommand[]>([]);
  const [draft, setDraft] = useState<string | null>(null);
  const [panel, setPanel] = useState<PanelKind | null>(null);
  const [filePanelPath, setFilePanelPath] = useState<string | null>(null);
  // The subagents the agents panel opens on: those of the plate last pressed.
  const [agentFocus, setAgentFocus] = useState<string[]>([]);
  const [sidebarOpen, setSidebarOpen] = useState(true);

  // Widths of the two columns a person drags. What is stored is what they
  // chose; what is drawn is that, fitted to the window as it is now.
  const windowWidth = useWindowWidth();
  const [sideChosen, setSideWidth] = useStoredWidth("yaca.width.side");
  const [listChosen, setListWidth] = useStoredWidth("yaca.width.panel");
  const [fileChosen, setFileWidth] = useStoredWidth("yaca.width.file");
  const sideLimits = sideBounds(windowWidth);
  const sideWidth = clamp(sideChosen ?? SIDE.initial, sideLimits);

  const failed = useCallback((e: unknown) => setError(String(e)), []);
  const chats = useChats(failed);
  const active = chats.active;

  const closePanel = useCallback(() => setPanel(null), []);
  const browserColumn = useBrowserColumn({
    session: active?.id ?? null,
    windowWidth,
    sideWidth: sidebarOpen ? sideWidth : 0,
    panelOpen: panel !== null,
    closePanel,
  });
  // The settings lie over the whole window, the browser's page included.
  useBrowserCover(settingsAt !== null);

  // Ручной скачок по ленте идёт через её окно: блок далеко от видимой области
  // не размечен, и `getElementById` на нём мёртв. Скачки тут (стрелки шапки) —
  // это и возврат по чекпоинтам, поэтому ручка одна и та же для обоих путей.
  const feedJump = useRef<FeedJump>(null);

  /**
   * Слэш-команды того проекта, в котором сейчас пишут.
   *
   * Список нужен только меню: разворачивает команду ядро, читая каталог на
   * каждом сообщении. Поэтому и перечитывается он по требованию композера, а
   * не подпиской: файлы проекта живут вне потока `AgentEvent`.
   */
  const project = active?.project ?? draft;
  // Картинки разговора по порядку — по ним листает просмотрщик.
  const activeFeed = active?.feed ?? null;
  const gallery = useMemo(() => galleryOf(activeFeed), [activeFeed]);
  const refreshCommands = useCallback(() => {
    if (project === null) return setSlashCommands([]);
    void Promise.all([projectCommands(project), projectSkills(project)]).then(
      ([commands, skills]) => setSlashCommands(withSkills(commands, skills.skills)),
      failed,
    );
  }, [project, failed]);

  useEffect(refreshCommands, [refreshCommands]);
  // The chat's own commands on top of the project's: its MCP servers' prompts.
  const activeMcp = active?.feed.mcp ?? null;
  const chatCommands = useMemo(
    () => [...slashCommands, ...mcpPrompts(activeMcp)],
    [slashCommands, activeMcp],
  );

  useEffect(() => {
    // Проекты помнят глобальные настройки приложения: в самом проекте список
    // хранить негде, он нужен раньше, чем известен хоть один проект.
    void listProjects().then((list) => {
      setProjects(list);
      setDraft((now) => now ?? list[0] ?? null);
    }, failed);
  }, [failed]);

  /** Модели меняются в панели провайдеров и перечитываются после неё. */
  const refreshProviders = useCallback(() => {
    // Ошибку глотать нельзя: битый `providers.json` останавливает список
    // моделей, и молча пустой переключатель не объяснил бы, почему он
    // пуст.
    void fetchModels().then(setModels, failed);
  }, [failed]);

  useEffect(refreshProviders, [refreshProviders]);

  // Провайдеров правят в окне настроек, и переключатель моделей
  // обязан это увидеть: список, оставшийся прежним после «Опросить», — это
  // окно, врущее про состояние.
  useEffect(() => {
    let live = true;
    const off = onProviders(() => live && refreshProviders());
    return () => {
      live = false;
      void off.then((stop) => stop());
    };
  }, [refreshProviders]);

  // The top bar's button shows and hides the panel as a whole. It comes back
  // on the tab it was closed on; before any was opened, that is processes.
  const lastPanel = useRef<PanelKind>("processes");
  useEffect(() => {
    if (panel !== null) lastPanel.current = panel;
  }, [panel]);
  const togglePanel = useCallback(
    () => setPanel((now) => (now === null ? lastPanel.current : null)),
    [],
  );

  // --- проекты и чаты -------------------------------------------------------

  /**
   * «Новый чат»: окно показывает домашний экран этого проекта.
   *
   * Сессия рождается первым сообщением, и кнопка, заводящая её сразу,
   * оставляла бы в списке пустую строку за каждое «передумал».
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
    answerElicitation: (requestId, reply) => send(commands.answerElicitation(requestId, reply)),
    answerQuestion: (reply: QuestionReply) => {
      if (active?.feed.question) {
        send(commands.answerQuestion(active.feed.question.id, reply));
      }
    },
    previewCheckpoint: (id) => send(commands.previewCheckpoint(id)),
    openCheckpoints: () => setPanel("checkpoints"),
    restoreCheckpoint: (id) => send(commands.restoreCheckpoint(id)),
    cancelPreview: () => {
      if (active) chats.patch(active.id, { feed: { ...active.feed, preview: null } });
    },
    editSummary: (text) => send(commands.editSummary(text)),
    // Панель файла открывается и по имени файла в строке действия.
    openFile: (path) => {
      setFilePanelPath(path);
      setPanel("file");
    },
    openAgents: (ids) => {
      setAgentFocus(ids);
      setPanel("agents");
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
      send(commands.send(text, attachments));
    },
    // The core's queue, drawn as it stands. Cancelling sends the id; the
    // list that comes back is what the chips show.
    cancelQueued: (id) => send(commands.cancelQueued(id)),
    // The `@` menu asks the core, and the answer comes back in the feed.
    searchFiles: (query) => send(commands.searchFiles(query)),
    stop: () => send(commands.cancel()),
    setMode: (mode) => send(commands.setMode(mode)),
    setBypass: (on) => send(commands.setBypass(on)),
    setModel: (model) => send(commands.setModel(model)),
    setEffort: (effort: Effort) => send(commands.setEffort(effort)),
    // Встроенные команды делает интерфейс — это те же действия, что у
    // полосы уведомлений и у строки «Новый чат» сайдбара.
    compact: () => send(commands.compact()),
    openCheckpoints: () => setPanel("checkpoints"),
    openMcp: () => setPanel("mcp"),
    openLsp: () => setPanel("lsp"),
    newChat: () => {
      if (active !== null) newChat(active.project);
    },
    // Пути отдаются от корня проекта — их же увидит модель, и карточка
    // вложения не растянется абсолютным путём.
    pickFiles: hasFileDialogs
      ? async () => {
          const root = active?.feed.state?.projectRoot;
          if (root === undefined) return [];
          const picked = await chooseFiles(root, t("composer.attach"));
          return picked.map((path) => relativeTo(root, path));
        }
      : undefined,
    ...imageActions(active?.feed.state?.projectRoot ?? null),
    refreshCommands,
  };

  // --- правая панель ----------------------------------------------------------

  const watchProcess = useCallback(
    (id: ProcessId | null) => {
      if (!active) return;
      chats.patch(active.id, { watched: id });
      chats.send(active.id, commands.watchProcess(id));
    },
    [active, chats],
  );

  // ⌥← / ⌥→ листают файлы текущего набора, не закрывая панель.
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
    // Esc belongs to an open question (it dismisses it), as it does to a permission.
    suspended: active?.feed.request != null || active?.feed.question != null,
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
      // некуда, а чип там свой и живёт в состоянии `Home`. Клавиатуру
      // до него не дотянули — она вся висит здесь, — и это видимая дыра, а не
      // забытая ветка: мышью режим до старта выбирается.
      const now = active?.feed.state?.mode;
      if (now === undefined) return;
      composerActions.setMode(MODES[(MODES.indexOf(now) + 1) % MODES.length]);
    },
  });

  // --- разметка -------------------------------------------------------------

  // Сайдбар один и тот же на обоих экранах: разное — то, что стоит справа.
  const sidebar = (collapsed: boolean) => (
    <Sidebar
      projects={projects}
      chats={chats.chats}
      activeId={active?.id ?? null}
      current={active?.project ?? draft}
      // Перечитать списки есть смысл, когда они могли измениться: открыли или
      // закрыли чат, закончился ход и сменился заголовок.
      reload={chats.chats.map((chat) => `${chat.id}:${chat.title}`).join("|")}
      onNewChat={newChat}
      onOpenSession={(project, id) => void chats.resume(project, id).catch(failed)}
      onCloseChat={(id) => void chats.close(id).catch(failed)}
      onAddProject={() => void addProject()}
      onForgetProject={(project) => void forgetProject(project)}
      onOpenSettings={() => setSettingsAt("providers")}
      width={sideWidth}
      bounds={sideLimits}
      onResize={setSideWidth}
      collapsed={collapsed}
    />
  );

  const settingsView = settingsAt !== null ? (
    <div className="settings-overlay">
      <Settings
        prefs={livePrefs}
        section={settingsAt}
        onSection={setSettingsAt}
        onClose={() => setSettingsAt(null)}
      />
    </div>
  ) : null;

  // Состояние приезжает вместе с открытым чатом, поэтому `null` здесь значит
  // ровно одно: открытого чата нет и окно на домашнем экране.
  const state = active?.feed.state ?? null;

  if (active === null || state === null) {
    return frame(
      <Pictures root={project} gallery={gallery}>
      <div className="shell">
        {/* На домашнем экране сайдбар не прячется: прячет его кнопка шапки,
            а шапки здесь нет. Прятать тут и нечего — кроме сайдбара на
            домашнем экране никакой навигации не осталось. */}
        {sidebar(false)}

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
          onOpenSettings={() => setSettingsAt("providers")}
        />
        {settingsView}
      </div>
      </Pictures>,
    );
  }

  const runningProcesses = active.feed.processes.filter(
    (p) => p.state.type === "running",
  ).length;

  // The panel takes what the sidebar and the feed's floor leave it. A file is
  // opened to be read and starts wide; the lists start as a narrow column.
  const panelLimits = panelBounds(
    windowWidth,
    (sidebarOpen ? sideWidth : 0) + browserColumn.reserve,
  );
  const panelInitial =
    panel === "file" ? Math.round(windowWidth * FILE_SHARE) : PANEL.initial;
  const panelChosen = (panel === "file" ? fileChosen : listChosen) ?? panelInitial;
  const panelWidth = clamp(panelChosen, panelLimits ?? { min: PANEL.min, max: windowWidth });

  return frame(
    <Pictures root={project} gallery={gallery}>
    <div className="shell">
      {sidebar(!sidebarOpen)}

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
          panelOpen={panel !== null}
          runningProcesses={runningProcesses}
          onToggleSidebar={() => setSidebarOpen((now) => !now)}
          onTogglePanel={togglePanel}
          browser={
            browserColumn.supported
              ? {
                  open: browserColumn.open,
                  busy: browserColumn.state?.busy ?? false,
                  onToggle: browserColumn.toggle,
                }
              : null
          }
        />

        <Feed feed={active.feed} actions={feedActions} ref={feedJump} />

        <div className="bars">
          <Notices
            feed={active.feed}
            onCompact={() => send(commands.compact())}
            onOpenMcp={() => setPanel("mcp")}
            onOpenLsp={() => setPanel("lsp")}
          />
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
          usage={active.feed.usage}
          models={
            state.provider
              ? models.filter((m) => !m.providerId || m.providerId === state.provider)
              : models
          }
          commands={chatCommands}
          queued={active.feed.queued}
          found={active.feed.found}
          actions={composerActions}
          // Черновик принадлежит разговору, а не проекту: чатов в проекте
          // несколько, и недописанное в одном не должно всплывать в другом.
          draftKey={`yaca.draft.${active.id}`}
        />
      </div>

      {browserColumn.open && (
        <BrowserColumn
          session={active.id}
          state={browserColumn.state}
          width={browserColumn.width}
          bounds={browserColumn.bounds}
          initialWidth={browserColumn.initialWidth}
          onResize={browserColumn.setWidth}
          onClose={browserColumn.close}
        />
      )}

      {panel !== null && (
        <PanelFrameContext.Provider
          value={{
            current: panel,
            filePath: filePanelPath,
            hasAgents: agentsOf(active.feed).length > 0,
            onSwitch: setPanel,
            onClose: () => setPanel(null),
            width: panelWidth,
            bounds: panelLimits,
            initialWidth: panelInitial,
            onResize: panel === "file" ? setFileWidth : setListWidth,
          }}
        >
          {panel === "processes" && (
            <ProcessesPanel
              feed={active.feed}
              watched={active.watched}
              onWatch={watchProcess}
              onStop={(id) => send(commands.stopProcess(id))}
            />
          )}

          {panel === "file" && filePanelPath !== null && (
            <FilePanel
              path={filePanelPath}
              feed={active.feed}
              onRead={(path) => send(commands.readFileView(path))}
            />
          )}

          {panel === "mcp" && (
            <McpPanel
              servers={active.feed.mcp}
              onRefresh={() => void chats.refreshMcp(active.id).catch(failed)}
              onReconnect={(server) => send(commands.reconnectMcpServer(server))}
              onApprove={(server) => send(commands.approveMcpServer(server))}
              onLogin={(server) => send(commands.loginMcpServer(server))}
              onLogout={(server) => send(commands.logoutMcpServer(server))}
            />
          )}

          {panel === "lsp" && (
            <LspPanel
              servers={active.feed.lsp}
              onRestart={(server) => send(commands.restartLspServer(server))}
            />
          )}

          {panel === "agents" && (
            <AgentsPanel
              feed={active.feed}
              focus={agentFocus}
              onOpenFile={feedActions.openFile}
            />
          )}

          {panel === "checkpoints" && (
            <CheckpointsPanel
              feed={active.feed}
              onPreview={feedActions.previewCheckpoint}
              onRestore={feedActions.restoreCheckpoint}
              onCancel={feedActions.cancelPreview}
            />
          )}
        </PanelFrameContext.Provider>
      )}
      {settingsView}
    </div>
    </Pictures>,
  );
}
