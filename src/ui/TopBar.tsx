/**
 * Верхняя панель.
 *
 * Держит навигацию и опознание сессии, а не состояние работы: состояние живёт
 * внизу, рядом с композером, куда и так смотрят. Марки в шапке нет — квадрат с
 * отражённой `C` живёт на домашнем экране и в иконке приложения.
 *
 * По краям две кнопки одной формы (`yaca-icons`): слева показать или спрятать
 * сайдбар, справа — правую панель. Иконка каждой показывает, открыта ли её
 * колонка; при правой стоит точка, пока работают фоновые команды. Кнопка открывает панель на
 * той вкладке, на которой её закрыли; сами вкладки переключаются в панели.
 * Стрелки листают ленту по сообщениям человека.
 */

import { ArrowLeft, ArrowRight } from "@phosphor-icons/react";
import { useState } from "react";

import { t } from "../i18n";
import { baseName } from "./format";
import { PanelLeft, PanelRight } from "./yaca-icons";

export function TopBar({
  title,
  projectRoot,
  canGoBack,
  canGoForward,
  onBack,
  onForward,
  onRename,
  sidebarOpen,
  panelOpen,
  runningProcesses,
  onToggleSidebar,
  onTogglePanel,
}: {
  title: string;
  projectRoot: string;
  canGoBack: boolean;
  canGoForward: boolean;
  onBack: () => void;
  onForward: () => void;
  onRename: (title: string) => void;
  /** Показана ли колонка сайдбара слева. */
  sidebarOpen: boolean;
  /** Открыта ли правая панель, на любой вкладке. */
  panelOpen: boolean;
  /** Точка при кнопке панели — пока есть живые фоновые команды. */
  runningProcesses: number;
  onToggleSidebar: () => void;
  onTogglePanel: () => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);

  return (
    <div className="top">
      <button
        className="top-icon"
        aria-pressed={sidebarOpen}
        onClick={onToggleSidebar}
        aria-label={t("side.toggle")}
        title={t("side.toggle")}
      >
        <PanelLeft size={18} open={sidebarOpen} />
      </button>

      <button
        className="top-icon"
        onClick={onBack}
        disabled={!canGoBack}
        aria-label={t("top.back")}
        title={t("top.back")}
      >
        <ArrowLeft size={16} />
      </button>
      <button
        className="top-icon"
        onClick={onForward}
        disabled={!canGoForward}
        aria-label={t("top.forward")}
        title={t("top.forward")}
      >
        <ArrowRight size={16} />
      </button>

      {editing === null ? (
        <div
          className="top-title"
          title={t("top.rename_hint")}
          onDoubleClick={() => setEditing(title)}
        >
          {title || t("top.untitled")}
        </div>
      ) : (
        <input
          className="top-title"
          autoFocus
          value={editing}
          onChange={(e) => setEditing(e.target.value)}
          onBlur={() => setEditing(null)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              onRename(editing);
              setEditing(null);
            }
            if (e.key === "Escape") setEditing(null);
          }}
        />
      )}

      <span className="top-project" title={projectRoot}>
        {baseName(projectRoot)}
      </span>

      <div className="top-gap" />

      <div className="top-icon-wrap">
        <button
          className="top-icon"
          aria-pressed={panelOpen}
          onClick={onTogglePanel}
          aria-label={t("panel.toggle")}
          title={t("panel.toggle")}
        >
          <PanelRight size={18} open={panelOpen} />
        </button>
        {runningProcesses > 0 && <span className="top-dot" aria-hidden="true" />}
      </div>
    </div>
  );
}
