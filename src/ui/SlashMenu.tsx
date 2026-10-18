/**
 * Меню слэш-команд §7.3 — дополнение над полем композера.
 *
 * Строится теми же классами, что и меню режима и модели: это тот же список
 * над тем же полем, и вторая внешность у него была бы без причины.
 *
 * Two columns: names on the left, and what the highlighted one does on the
 * right. A skill's description is a paragraph written for the model, and in
 * every row it made the list a wall nobody reads; one at a time, beside the
 * name it belongs to, it is read. The pane follows the highlight, which the
 * mouse moves as well as the arrows, so it explains whatever the next `⏎`
 * would take.
 *
 * Выбор берётся мышью на `mousedown`, а не на `click`: клик сначала уводит
 * фокус из поля, и человек, дополнивший команду мышью, оказывался бы с
 * дополненной строкой и без курсора в ней.
 */

import { useEffect, useRef } from "react";

import { t, type Key } from "../i18n";
import type { SlashItem } from "./slash";

export function SlashMenu({
  items,
  at,
  onPick,
  onHover,
}: {
  items: SlashItem[];
  /** Подсвеченная строка: её берут `⇥` и `⏎`. */
  at: number;
  onPick: (item: SlashItem) => void;
  /** The mouse moved the highlight. */
  onHover: (index: number) => void;
}) {
  const list = useRef<HTMLDivElement>(null);

  // Arrows can take the highlight past the visible rows; it is brought back
  // into view rather than left to be found by scrolling.
  useEffect(() => {
    const row = list.current?.children[at];
    if (row instanceof HTMLElement) row.scrollIntoView({ block: "nearest" });
  }, [at]);

  const current = items[at];
  return (
    <div className="menu menu-slash">
      <div className="slash-list" role="listbox" ref={list}>
        {items.map((item, index) => (
          <button
            className="menu-item"
            key={item.name}
            role="option"
            aria-selected={index === at}
            onMouseEnter={() => onHover(index)}
            onMouseDown={(event) => {
              event.preventDefault();
              onPick(item);
            }}
          >
            <span className="mono-target slash-name">/{item.name}</span>
            {/* A built-in's note is a few words, and the name alone says less. */}
            {item.builtin !== null && <span className="menu-note">{item.description}</span>}
          </button>
        ))}
      </div>
      {current !== undefined && (
        <div className="slash-detail" aria-live="polite">
          <div className="slash-detail-name">/{current.name}</div>
          <div className="slash-detail-kind">{t(kindKey(current))}</div>
          <div className="slash-detail-text">
            {current.description.trim() === "" ? t("slash.no_description") : current.description}
          </div>
        </div>
      )}
    </div>
  );
}

function kindKey(item: SlashItem): Key {
  switch (item.kind) {
    case "builtin":
      return "slash.kind_builtin";
    case "command":
      return "slash.kind_command";
    case "pluginCommand":
      return "slash.kind_plugin_command";
    case "mcpPrompt":
      return "slash.kind_mcp_prompt";
    case "skill":
      return item.scope === "user"
        ? "slash.kind_skill_user"
        : item.scope === "plugin"
          ? "slash.kind_skill_plugin"
          : "slash.kind_skill_project";
  }
}
