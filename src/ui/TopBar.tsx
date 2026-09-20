/**
 * Верхняя панель §3.
 *
 * Держит навигацию и опознание сессии, а не состояние работы: состояние живёт
 * внизу, рядом с композером, куда и так смотрят. Марки в шапке нет — квадрат с
 * отражённой `C` живёт на домашнем экране и в иконке приложения.
 *
 * Слева `List` — показать или спрятать сайдбар: список сессий уехал из
 * выдвижной панели §1.1 в свою колонку, и кнопка теперь про колонку, а не про
 * панель. Справа `SquaresFour` процессов с точкой при активных. Панель файла
 * открывается из ленты и полосы, а меню, поиск, диктовка и настройки ждут
 * своих подсистем: кнопка, которая ничего не открывает, это интерфейс,
 * врущий про состояние (§0). Стрелки листают ленту по сообщениям человека, и
 * это работает уже сейчас.
 */

import { ArrowLeft, ArrowRight, List, SquaresFour } from "@phosphor-icons/react";
import { useState } from "react";

import { t } from "../i18n";
import { baseName } from "./format";

export function TopBar({
  title,
  projectRoot,
  canGoBack,
  canGoForward,
  onBack,
  onForward,
  onRename,
  sidebarOpen,
  panel,
  runningProcesses,
  onToggleSidebar,
  onToggleProcesses,
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
  /** Какая панель §1.1 открыта кнопкой шапки. */
  panel: "processes" | null;
  /** Точка при кнопке процессов — пока есть живые фоновые команды (§3). */
  runningProcesses: number;
  onToggleSidebar: () => void;
  onToggleProcesses: () => void;
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
        <List size={16} />
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
          data-panel-trigger
          aria-pressed={panel === "processes"}
          onClick={onToggleProcesses}
          aria-label={t("top.processes")}
          title={t("top.processes")}
        >
          <SquaresFour size={16} />
        </button>
        {runningProcesses > 0 && <span className="top-dot" aria-hidden="true" />}
      </div>
    </div>
  );
}
