/**
 * The window's own title bar (`core/frame`): the mark and the name, the Help
 * menu, the empty stretch the window is dragged by, and the three buttons.
 *
 * It replaces two strips of the system's — the title and, under it, a menu
 * bar holding the single word "Help" — with one.
 *
 * Dragging is Tauri's: an element marked `data-tauri-drag-region` moves the
 * window and maximizes it on a double click. The mark applies to the element
 * itself and not to what is inside it, so the stretches meant for dragging
 * each carry it, and the menu and the buttons, which do not, stay clickable.
 */

import { CopySimple, Minus, Square, X } from "@phosphor-icons/react";
import { DropdownMenu } from "radix-ui";
import { useEffect, useState } from "react";

import { windowControls, type WindowControls } from "../core/frame";
import { t } from "../i18n";
import { YacaMark } from "./Logo";

import "./popmenu.css";
import "./titlebar.css";

export function TitleBar({
  onCheckUpdates,
  onAbout,
}: {
  onCheckUpdates: () => void;
  onAbout: () => void;
}) {
  const [controls, setControls] = useState<WindowControls | null>(null);
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let live = true;
    void windowControls().then((answer) => {
      if (live) setControls(answer);
    });
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => controls?.watchMaximized(setMaximized), [controls]);

  return (
    <header className="title" data-tauri-drag-region>
      <div className="title-brand" data-tauri-drag-region>
        <YacaMark size={15} />
        Yaca
      </div>

      <DropdownMenu.Root>
        <DropdownMenu.Trigger className="title-menu">{t("menu.help")}</DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content className="pop" align="start" sideOffset={2}>
            <DropdownMenu.Item className="pop-item" onSelect={onCheckUpdates}>
              {t("menu.check_updates")}
            </DropdownMenu.Item>
            <DropdownMenu.Item className="pop-item" onSelect={onAbout}>
              {t("menu.about")}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <div className="title-gap" data-tauri-drag-region />

      {controls !== null && (
        <div className="title-controls">
          <button
            type="button"
            className="title-button"
            aria-label={t("title.minimize")}
            title={t("title.minimize")}
            onClick={controls.minimize}
          >
            <Minus size={14} />
          </button>
          <button
            type="button"
            className="title-button"
            aria-label={maximized ? t("title.restore") : t("title.maximize")}
            title={maximized ? t("title.restore") : t("title.maximize")}
            onClick={controls.toggleMaximize}
          >
            {maximized ? <CopySimple size={13} /> : <Square size={12} />}
          </button>
          <button
            type="button"
            className="title-button title-close"
            aria-label={t("title.close")}
            title={t("title.close")}
            onClick={controls.close}
          >
            <X size={14} />
          </button>
        </div>
      )}
    </header>
  );
}
